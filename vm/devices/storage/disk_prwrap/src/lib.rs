// Copyright (C) Microsoft Corporation. All rights reserved.

//! Provides a basic implementation of SCSI persistent reservations on top of
//! any other disk type.
//!
//! Since these reservations are stored locally in memory, this is not useful
//! for actually sharing a disk between VMs. This is just useful for testing.

use async_trait::async_trait;
use disk_backend::pr;
use disk_backend::pr::ReservationType;
use disk_backend::resolve::ResolveDiskParameters;
use disk_backend::resolve::ResolvedSimpleDisk;
use disk_backend::AsyncDisk;
use disk_backend::DiskError;
use disk_backend::SimpleDisk;
use disk_backend_resources::DiskWithReservationsHandle;
use inspect::Inspect;
use parking_lot::Mutex;
use scsi_buffers::RequestBuffers;
use stackfuture::StackFuture;
use std::num::NonZeroU64;
use std::num::Wrapping;
use std::sync::Arc;
use vm_resource::declare_static_async_resolver;
use vm_resource::kind::DiskHandleKind;
use vm_resource::AsyncResolveResource;
use vm_resource::ResolveError;
use vm_resource::ResourceResolver;

pub struct DiskWithReservationsResolver;
declare_static_async_resolver!(
    DiskWithReservationsResolver,
    (DiskHandleKind, DiskWithReservationsHandle)
);

#[async_trait]
impl AsyncResolveResource<DiskHandleKind, DiskWithReservationsHandle>
    for DiskWithReservationsResolver
{
    type Output = ResolvedSimpleDisk;
    type Error = ResolveError;

    async fn resolve(
        &self,
        resolver: &ResourceResolver,
        rsrc: DiskWithReservationsHandle,
        input: ResolveDiskParameters<'_>,
    ) -> Result<Self::Output, Self::Error> {
        let inner = resolver.resolve(rsrc.0, input).await?;
        Ok(DiskWithReservations::new(inner.0).into())
    }
}

/// A disk wrapper that adds persistent reservations support to any disk type.
///
/// The reservations are handled locally in memory, so they cannot be used to
/// actually share a disk.
#[derive(Inspect)]
pub struct DiskWithReservations {
    inner: Arc<dyn SimpleDisk>,
    #[inspect(flatten)]
    state: Mutex<ReservationState>,
}

#[derive(Default, Debug, Inspect)]
struct ReservationState {
    generation: Wrapping<u32>,
    registered_key: Option<NonZeroU64>,
    reservation_type: Option<ReservationType>,
    persist_through_power_loss: bool,
}

impl DiskWithReservations {
    /// Wraps `inner` with persistent reservations support.
    pub fn new(inner: Arc<dyn SimpleDisk>) -> Self {
        Self {
            inner,
            state: Default::default(),
        }
    }
}

impl SimpleDisk for DiskWithReservations {
    fn disk_type(&self) -> &str {
        "prwrap"
    }

    fn sector_count(&self) -> u64 {
        self.inner.sector_count()
    }

    fn sector_size(&self) -> u32 {
        self.inner.sector_size()
    }

    fn disk_id(&self) -> Option<[u8; 16]> {
        self.inner.disk_id()
    }

    fn physical_sector_size(&self) -> u32 {
        self.inner.physical_sector_size()
    }

    fn is_fua_respected(&self) -> bool {
        self.inner.is_fua_respected()
    }

    fn is_read_only(&self) -> bool {
        self.inner.is_read_only()
    }

    fn unmap(&self) -> Option<&dyn disk_backend::Unmap> {
        None // TODO
    }

    fn lba_status(&self) -> Option<&dyn disk_backend::GetLbaStatus> {
        self.inner.lba_status()
    }

    fn pr(&self) -> Option<&dyn pr::PersistentReservation> {
        Some(self)
    }
}

#[async_trait::async_trait]
impl AsyncDisk for DiskWithReservations {
    fn read_vectored<'a>(
        &'a self,
        buffers: &'a RequestBuffers<'a>,
        sector: u64,
    ) -> StackFuture<'a, Result<(), DiskError>, { disk_backend::ASYNC_DISK_STACK_SIZE }> {
        self.inner.read_vectored(buffers, sector)
    }

    fn write_vectored<'a>(
        &'a self,
        buffers: &'a RequestBuffers<'a>,
        sector: u64,
        fua: bool,
    ) -> StackFuture<'a, Result<(), DiskError>, { disk_backend::ASYNC_DISK_STACK_SIZE }> {
        self.inner.write_vectored(buffers, sector, fua)
    }

    fn sync_cache(
        &self,
    ) -> StackFuture<'_, Result<(), DiskError>, { disk_backend::ASYNC_DISK_STACK_SIZE }> {
        self.inner.sync_cache()
    }
}

#[async_trait]
impl pr::PersistentReservation for DiskWithReservations {
    fn capabilities(&self) -> pr::ReservationCapabilities {
        pr::ReservationCapabilities {
            write_exclusive: true,
            exclusive_access: true,
            write_exclusive_registrants_only: true,
            exclusive_access_registrants_only: true,
            write_exclusive_all_registrants: false,
            exclusive_access_all_registrants: false,
            persist_through_power_loss: true,
        }
    }

    async fn report(&self) -> Result<pr::ReservationReport, DiskError> {
        tracing::info!("reading full status");
        let state = self.state.lock();
        let report = pr::ReservationReport {
            generation: state.generation.0,
            reservation_type: state.reservation_type,
            persist_through_power_loss: state.persist_through_power_loss,
            controllers: state
                .registered_key
                .iter()
                .map(|&key| pr::RegisteredController {
                    key: key.get(),
                    host_id: vec![0; 8],
                    controller_id: 0,
                    holds_reservation: state.reservation_type.is_some(),
                })
                .collect(),
        };
        Ok(report)
    }

    async fn register(
        &self,
        current_key: Option<u64>,
        new_key: u64,
        ptpl: Option<bool>,
    ) -> Result<(), DiskError> {
        let mut state = self.state.lock();
        if let Some(current_key) = current_key {
            if state.registered_key != NonZeroU64::new(current_key) {
                return Err(DiskError::ReservationConflict);
            }
        }
        let new_key = NonZeroU64::new(new_key);
        state.registered_key = new_key;
        if new_key.is_none() {
            state.reservation_type = None;
        }
        if let Some(ptpl) = ptpl {
            state.persist_through_power_loss = ptpl;
        }
        state.generation += 1;
        Ok(())
    }

    async fn reserve(&self, key: u64, reservation_type: ReservationType) -> Result<(), DiskError> {
        let mut state = self.state.lock();
        if state.registered_key.is_none()
            || state.registered_key != NonZeroU64::new(key)
            || (state.reservation_type.is_some()
                && state.reservation_type != Some(reservation_type))
        {
            return Err(DiskError::ReservationConflict);
        }
        state.reservation_type = Some(reservation_type);
        Ok(())
    }

    async fn release(&self, key: u64, reservation_type: ReservationType) -> Result<(), DiskError> {
        let mut state = self.state.lock();
        if state.registered_key.is_none() || state.registered_key != NonZeroU64::new(key) {
            return Err(DiskError::ReservationConflict);
        }

        if state.reservation_type.is_some() {
            if state.reservation_type != Some(reservation_type) {
                return Err(DiskError::InvalidInput);
            }
            state.reservation_type = None;
        }
        Ok(())
    }

    async fn clear(&self, key: u64) -> Result<(), DiskError> {
        let mut state = self.state.lock();
        if state.registered_key.is_none() || state.registered_key != NonZeroU64::new(key) {
            return Err(DiskError::ReservationConflict);
        }
        state.registered_key = None;
        state.reservation_type = None;
        state.generation += 1;
        Ok(())
    }

    async fn preempt(
        &self,
        current_key: u64,
        preempt_key: u64,
        reservation_type: ReservationType,
        _abort: bool,
    ) -> Result<(), DiskError> {
        let mut state = self.state.lock();
        if state.registered_key.is_none() || state.registered_key != NonZeroU64::new(current_key) {
            return Err(DiskError::ReservationConflict);
        }
        if state.registered_key != NonZeroU64::new(preempt_key)
            || (state.reservation_type.is_some()
                && state.reservation_type != Some(reservation_type))
        {
            return Err(DiskError::InvalidInput);
        }

        state.reservation_type = None;
        state.generation += 1;
        Ok(())
    }
}