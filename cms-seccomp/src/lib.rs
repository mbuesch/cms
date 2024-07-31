// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![forbid(unsafe_code)]

use anyhow::{self as ah, Context as _};
use seccompiler::{
    apply_filter_all_threads, sock_filter, BpfProgram, SeccompAction, SeccompFilter,
};
use std::{collections::BTreeMap, env::consts::ARCH};

macro_rules! sys {
    ($ident:ident) => {{
        #[allow(clippy::useless_conversion)]
        let id: i64 = libc::$ident.into();
        id
    }};
}

/// Returns `true` if seccomp is supported on this platform.
pub fn seccomp_supported() -> bool {
    // This is what `seccompiler` currently supports:
    cfg!(any(target_arch = "x86_64", target_arch = "aarch64"))
}

/// Abstract allow-list features that map to one or more syscalls each.
#[derive(Clone, Debug)]
pub enum Allow {
    Mmap,
    Mprotect,
    UnixConnect,
    UnixListen,
    Open,
    Read,
    Write,
    Stat,
    Listdir,
    Recv,
    Send,
    Sendfile,
    Futex,
    Signal,
    Threading,
    Inotify,
    Prctl,
    Timer,
    ClockGet,
    ClockSet,
    Sleep,
}

/// Action to be performed, if a syscall is executed that is not in the allow-list.
#[derive(Clone, Debug)]
pub enum Action {
    /// Kill the process.
    Kill,
    /// Only log the event and keep running. See the kernel logs.
    Log,
}

pub struct Filter(BpfProgram);

impl Filter {
    /// Simple serialization, without serde.
    pub fn serialize(&self) -> Vec<u8> {
        let mut raw = Vec::with_capacity(self.0.len() * 8);
        for insn in &self.0 {
            raw.extend_from_slice(&insn.code.to_le_bytes());
            raw.push(insn.jt);
            raw.push(insn.jf);
            raw.extend_from_slice(&insn.k.to_le_bytes());
        }
        debug_assert_eq!(raw.len(), self.0.len() * 8);
        raw
    }

    /// Simple de-serialization, without serde.
    pub fn deserialize(raw: &[u8]) -> Self {
        assert!(raw.len() % 8 == 0);
        let mut bpf = Vec::with_capacity(raw.len() / 8);
        for i in (0..raw.len()).step_by(8) {
            let code = u16::from_le_bytes(raw[i..i + 2].try_into().unwrap());
            let jt = raw[i + 2];
            let jf = raw[i + 3];
            let k = u32::from_le_bytes(raw[i + 4..i + 8].try_into().unwrap());
            bpf.push(sock_filter { code, jt, jf, k });
        }
        debug_assert_eq!(bpf.len() * 8, raw.len());
        Self(bpf)
    }

    pub fn compile(allow: &[Allow], deny_action: Action) -> ah::Result<Self> {
        Self::compile_for_arch(allow, deny_action, ARCH)
    }

    pub fn compile_for_arch(
        allow: &[Allow],
        deny_action: Action,
        arch: &str,
    ) -> ah::Result<Filter> {
        let mut rules: BTreeMap<_, _> = [
            (sys!(SYS_brk), vec![]),
            (sys!(SYS_close), vec![]),
            #[cfg(not(target_os = "android"))]
            (sys!(SYS_close_range), vec![]),
            (sys!(SYS_exit), vec![]),
            (sys!(SYS_exit_group), vec![]),
            (sys!(SYS_getpid), vec![]),
            (sys!(SYS_getrandom), vec![]),
            (sys!(SYS_gettid), vec![]),
            (sys!(SYS_madvise), vec![]),
            (sys!(SYS_munmap), vec![]),
            (sys!(SYS_sched_getaffinity), vec![]),
            (sys!(SYS_sigaltstack), vec![]),
            (sys!(SYS_gettimeofday), vec![]),
        ]
        .into();

        let add_read_write_rules = |rules: &mut BTreeMap<_, _>| {
            rules.insert(sys!(SYS_epoll_create1), vec![]);
            rules.insert(sys!(SYS_epoll_ctl), vec![]);
            rules.insert(sys!(SYS_epoll_pwait), vec![]);
            #[cfg(all(any(target_arch = "x86_64", target_arch = "arm"), target_os = "linux"))]
            rules.insert(sys!(SYS_epoll_pwait2), vec![]);
            rules.insert(sys!(SYS_epoll_wait), vec![]);
            rules.insert(sys!(SYS_lseek), vec![]);
            rules.insert(sys!(SYS_ppoll), vec![]);
            rules.insert(sys!(SYS_pselect6), vec![]);
        };

        for allow in allow {
            match *allow {
                Allow::Mmap => {
                    #[cfg(any(
                        target_arch = "x86",
                        target_arch = "x86_64",
                        target_arch = "aarch64"
                    ))]
                    rules.insert(sys!(SYS_mmap), vec![]);
                    #[cfg(any(target_arch = "x86", target_arch = "arm"))]
                    rules.insert(sys!(SYS_mmap2), vec![]);
                    rules.insert(sys!(SYS_mremap), vec![]);
                    rules.insert(sys!(SYS_munmap), vec![]);
                }
                Allow::Mprotect => {
                    rules.insert(sys!(SYS_mprotect), vec![]);
                }
                Allow::UnixConnect => {
                    rules.insert(sys!(SYS_connect), vec![]);
                    rules.insert(sys!(SYS_socket), vec![]); //TODO: Restrict to AF_UNIX
                    rules.insert(sys!(SYS_getsockopt), vec![]);
                }
                Allow::UnixListen => {
                    rules.insert(sys!(SYS_accept4), vec![]);
                    rules.insert(sys!(SYS_bind), vec![]);
                    rules.insert(sys!(SYS_listen), vec![]);
                    rules.insert(sys!(SYS_socket), vec![]); //TODO: Restrict to AF_UNIX
                    rules.insert(sys!(SYS_getsockopt), vec![]);
                }
                Allow::Open => {
                    //TODO: This should be restricted
                    rules.insert(sys!(SYS_open), vec![]);
                    rules.insert(sys!(SYS_openat), vec![]);
                }
                Allow::Read => {
                    rules.insert(sys!(SYS_pread64), vec![]);
                    rules.insert(sys!(SYS_preadv2), vec![]);
                    rules.insert(sys!(SYS_read), vec![]);
                    rules.insert(sys!(SYS_readv), vec![]);
                    add_read_write_rules(&mut rules);
                }
                Allow::Write => {
                    rules.insert(sys!(SYS_fdatasync), vec![]);
                    rules.insert(sys!(SYS_fsync), vec![]);
                    rules.insert(sys!(SYS_pwrite64), vec![]);
                    rules.insert(sys!(SYS_pwritev2), vec![]);
                    rules.insert(sys!(SYS_write), vec![]);
                    rules.insert(sys!(SYS_writev), vec![]);
                    add_read_write_rules(&mut rules);
                }
                Allow::Stat => {
                    rules.insert(sys!(SYS_fstat), vec![]);
                    rules.insert(sys!(SYS_statx), vec![]);
                    rules.insert(sys!(SYS_newfstatat), vec![]);
                }
                Allow::Listdir => {
                    rules.insert(sys!(SYS_getdents64), vec![]);
                }
                Allow::Recv => {
                    rules.insert(sys!(SYS_recvfrom), vec![]);
                    rules.insert(sys!(SYS_recvmsg), vec![]);
                    rules.insert(sys!(SYS_recvmmsg), vec![]);
                }
                Allow::Send => {
                    rules.insert(sys!(SYS_sendto), vec![]);
                    rules.insert(sys!(SYS_sendmsg), vec![]);
                    rules.insert(sys!(SYS_sendmmsg), vec![]);
                }
                Allow::Sendfile => {
                    rules.insert(sys!(SYS_sendfile), vec![]);
                }
                Allow::Futex => {
                    rules.insert(sys!(SYS_futex), vec![]);
                    rules.insert(sys!(SYS_get_robust_list), vec![]);
                    rules.insert(sys!(SYS_set_robust_list), vec![]);
                    #[cfg(all(
                        any(target_arch = "x86", target_arch = "x86_64", target_arch = "arm"),
                        target_os = "linux"
                    ))]
                    rules.insert(sys!(SYS_futex_waitv), vec![]);
                    //rules.insert(sys!(SYS_futex_wake), vec![]);
                    //rules.insert(sys!(SYS_futex_wait), vec![]);
                    //rules.insert(sys!(SYS_futex_requeue), vec![]);
                }
                Allow::Signal => {
                    rules.insert(sys!(SYS_rt_sigreturn), vec![]);
                    rules.insert(sys!(SYS_rt_sigprocmask), vec![]);
                }
                Allow::Threading => {
                    rules.insert(sys!(SYS_clone3), vec![]); //TODO restrict to threads
                    rules.insert(sys!(SYS_rseq), vec![]);
                }
                Allow::Inotify => {
                    rules.insert(sys!(SYS_inotify_init), vec![]);
                    rules.insert(sys!(SYS_inotify_add_watch), vec![]);
                    rules.insert(sys!(SYS_inotify_rm_watch), vec![]);
                }
                Allow::Prctl => {
                    //TODO: This should be restricted
                    rules.insert(sys!(SYS_prctl), vec![]);
                }
                Allow::Timer => {
                    rules.insert(sys!(SYS_timer_create), vec![]);
                    rules.insert(sys!(SYS_timer_settime), vec![]);
                    rules.insert(sys!(SYS_timer_gettime), vec![]);
                    rules.insert(sys!(SYS_timer_getoverrun), vec![]);
                    rules.insert(sys!(SYS_timer_delete), vec![]);
                }
                Allow::ClockGet => {
                    rules.insert(sys!(SYS_clock_gettime), vec![]);
                    rules.insert(sys!(SYS_clock_getres), vec![]);
                }
                Allow::ClockSet => {
                    rules.insert(sys!(SYS_clock_settime), vec![]);
                }
                Allow::Sleep => {
                    rules.insert(sys!(SYS_nanosleep), vec![]);
                    rules.insert(sys!(SYS_clock_nanosleep), vec![]);
                }
            }
        }

        let filter = SeccompFilter::new(
            rules,
            match deny_action {
                Action::Kill => SeccompAction::KillProcess,
                Action::Log => SeccompAction::Log,
            },
            SeccompAction::Allow,
            arch.try_into().context("Unsupported CPU ARCH")?,
        )
        .context("Create seccomp filter")?;

        let filter: BpfProgram = filter.try_into().context("Seccomp to BPF")?;

        Ok(Filter(filter))
    }

    pub fn install(&self) -> ah::Result<()> {
        apply_filter_all_threads(&self.0).context("Apply seccomp filter")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_filter_serialize() {
        let filter = Filter::compile(&[Allow::Read], Action::Kill).unwrap();
        let filter2 = Filter::deserialize(&filter.serialize());
        assert_eq!(filter.0.len(), filter2.0.len());
        for i in 0..filter.0.len() {
            assert_eq!(filter.0[i], filter2.0[i]);
        }
    }
}

// vim: ts=4 sw=4 expandtab
