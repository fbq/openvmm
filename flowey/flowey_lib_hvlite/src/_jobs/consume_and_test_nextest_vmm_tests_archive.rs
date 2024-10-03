// Copyright (C) Microsoft Corporation. All rights reserved.

//! Run a pre-built cargo-nextest based VMM tests archive.

use crate::run_cargo_nextest_run::NextestProfile;
use flowey::node::prelude::*;

#[derive(Serialize, Deserialize)]
pub struct VmmTestsDepArtifacts {
    pub artifact_dir_openvmm: Option<ReadVar<PathBuf>>,
    pub artifact_dir_pipette_windows: Option<ReadVar<PathBuf>>,
    pub artifact_dir_pipette_linux_musl: Option<ReadVar<PathBuf>>,
    pub artifact_dir_guest_test_uefi: Option<ReadVar<PathBuf>>,
    pub artifact_dir_openhcl_igvm_files: Option<ReadVar<PathBuf>>,
}

flowey_request! {
    pub struct Params {
        /// Friendly label for report JUnit test results
        pub junit_test_label: String,
        /// Existing VMM tests archive artifact dir
        pub vmm_tests_artifact_dir: ReadVar<PathBuf>,
        /// What target VMM tests were compiled for (determines required deps).
        pub target: target_lexicon::Triple,
        /// Nextest profile to use when running the source code
        pub nextest_profile: NextestProfile,
        /// Nextest test filter expression.
        pub nextest_filter_expr: Option<String>,
        /// Artifacts corresponding to required test dependencies
        pub dep_artifact_dirs: VmmTestsDepArtifacts,

        /// Whether the job should fail if any test has failed
        pub fail_job_on_test_fail: bool,
        pub done: WriteVar<SideEffect>,
    }
}

new_simple_flow_node!(struct Node);

impl SimpleFlowNode for Node {
    type Request = Params;

    fn imports(ctx: &mut ImportCtx<'_>) {
        ctx.import::<crate::artifact_guest_test_uefi::resolve::Node>();
        ctx.import::<crate::artifact_nextest_vmm_tests_archive::resolve::Node>();
        ctx.import::<crate::artifact_openhcl_igvm_from_recipe_extras::resolve::Node>();
        ctx.import::<crate::artifact_openhcl_igvm_from_recipe::resolve::Node>();
        ctx.import::<crate::artifact_openvmm::resolve::Node>();
        ctx.import::<crate::artifact_pipette::resolve::Node>();
        ctx.import::<crate::download_openvmm_vmm_tests_vhds::Node>();
        ctx.import::<crate::init_openvmm_magicpath_uefi_mu_msvm::Node>();
        ctx.import::<crate::init_vmm_tests_env::Node>();
        ctx.import::<crate::test_nextest_vmm_tests_archive::Node>();
        ctx.import::<flowey_lib_common::junit_publish_test_results::Node>();
    }

    fn process_request(request: Self::Request, ctx: &mut NodeCtx<'_>) -> anyhow::Result<()> {
        let Params {
            junit_test_label,
            vmm_tests_artifact_dir,
            target,
            nextest_profile,
            nextest_filter_expr,
            dep_artifact_dirs,
            fail_job_on_test_fail,
            done,
        } = request;

        let nextest_archive_file =
            ctx.reqv(
                |v| crate::artifact_nextest_vmm_tests_archive::resolve::Request {
                    artifact_dir: vmm_tests_artifact_dir,
                    nextest_archive: v,
                },
            );

        // use an ad-hoc, step-local dir as a staging ground for test content
        let test_content_dir = ctx.emit_rust_stepv("creating new test content dir", |_| {
            |_| Ok(std::env::current_dir()?.absolute()?)
        });

        let VmmTestsDepArtifacts {
            artifact_dir_openvmm,
            artifact_dir_pipette_windows,
            artifact_dir_pipette_linux_musl,
            artifact_dir_guest_test_uefi,
            artifact_dir_openhcl_igvm_files,
        } = dep_artifact_dirs;

        let register_openvmm = artifact_dir_openvmm.map(|artifact_dir| {
            ctx.reqv(|v| crate::artifact_openvmm::resolve::Request {
                artifact_dir,
                openvmm: v,
            })
        });

        let register_pipette_windows = artifact_dir_pipette_windows.map(|artifact_dir| {
            ctx.reqv(|v| crate::artifact_pipette::resolve::Request {
                artifact_dir,
                pipette: v,
            })
        });

        let register_pipette_linux_musl = artifact_dir_pipette_linux_musl.map(|artifact_dir| {
            ctx.reqv(|v| crate::artifact_pipette::resolve::Request {
                artifact_dir,
                pipette: v,
            })
        });

        let register_guest_test_uefi = artifact_dir_guest_test_uefi.map(|artifact_dir| {
            ctx.reqv(|v| crate::artifact_guest_test_uefi::resolve::Request {
                artifact_dir,
                guest_test_uefi: v,
            })
        });

        let register_openhcl_igvm_files = artifact_dir_openhcl_igvm_files.map(|artifact_dir| {
            ctx.reqv(
                |v| crate::artifact_openhcl_igvm_from_recipe::resolve::Request {
                    artifact_dir,
                    igvm_files: v,
                },
            )
        });

        // FIXME: share this with build_and_run_nextest_vmm_tests
        let disk_images_dir = Some({
            ctx.requests::<crate::download_openvmm_vmm_tests_vhds::Node>([
                crate::download_openvmm_vmm_tests_vhds::Request::DownloadVhds(vec![
                    vmm_test_images::KnownVhd::FreeBsd13_2,
                    vmm_test_images::KnownVhd::Gen1WindowsDataCenterCore2022,
                    vmm_test_images::KnownVhd::Gen2WindowsDataCenterCore2022,
                    vmm_test_images::KnownVhd::Ubuntu2204Server,
                ]),
            ]);

            ctx.requests::<crate::download_openvmm_vmm_tests_vhds::Node>([
                crate::download_openvmm_vmm_tests_vhds::Request::DownloadIsos(vec![
                    vmm_test_images::KnownIso::FreeBsd13_2,
                ]),
            ]);

            ctx.reqv(crate::download_openvmm_vmm_tests_vhds::Request::GetDownloadFolder)
        });

        // FUTURE: once we move away from the known_paths resolver, this will no
        // longer be an ambient pre-run dependency.
        let pre_run_deps =
            vec![
                ctx.reqv(|v| crate::init_openvmm_magicpath_uefi_mu_msvm::Request {
                    arch: crate::download_uefi_mu_msvm::MuMsvmArch::X86_64,
                    done: v,
                }),
            ];

        let extra_env = ctx.reqv(|v| crate::init_vmm_tests_env::Request {
            test_content_dir,
            vmm_tests_target: target.clone(),
            register_openvmm,
            register_pipette_windows,
            register_pipette_linux_musl,
            register_guest_test_uefi,
            disk_images_dir,
            register_openhcl_igvm_files,
            get_test_log_path: None,
            get_openhcl_dump_path: None,
            get_env: v,
        });

        let results = ctx.reqv(|v| crate::test_nextest_vmm_tests_archive::Request {
            nextest_archive_file,
            nextest_profile,
            nextest_filter_expr,
            extra_env,
            pre_run_deps,
            results: v,
        });

        let junit_xml = results.map(ctx, |r| r.junit_xml);
        let reported_results =
            ctx.reqv(
                |v| flowey_lib_common::junit_publish_test_results::Request::Register {
                    junit_xml,
                    test_label: junit_test_label,
                    done: v,
                },
            );

        ctx.emit_rust_step("report test results to overall pipeline status", |ctx| {
            reported_results.claim(ctx);
            done.claim(ctx);

            let results = results.clone().claim(ctx);
            move |rt| {
                let results = rt.read(results);
                if results.all_tests_passed {
                    log::info!("all tests passed!");
                } else {
                    if fail_job_on_test_fail {
                        anyhow::bail!("encountered test failures.")
                    } else {
                        log::error!("encountered test failures.")
                    }
                }

                Ok(())
            }
        });

        Ok(())
    }
}