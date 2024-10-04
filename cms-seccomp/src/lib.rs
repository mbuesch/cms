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
    apply_filter_all_threads, sock_filter, BpfProgram, SeccompAction, SeccompCmpArgLen,
    SeccompCmpOp, SeccompCondition, SeccompFilter, SeccompRule,
};
use std::{collections::BTreeMap, env::consts::ARCH};

macro_rules! sys {
    ($ident:ident) => {{
        #[allow(clippy::useless_conversion)]
        let id: i64 = libc::$ident.into();
        id
    }};
}

macro_rules! args {
    ($($arg:literal == $value:expr),*) => {
        SeccompRule::new(
            vec![
                $(
                    SeccompCondition::new(
                        $arg,
                        SeccompCmpArgLen::Dword,
                        SeccompCmpOp::Eq,
                        ($value) as _,
                    )?,
                )*
            ]
        )?
    };
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
        type RulesMap = BTreeMap<i64, Vec<SeccompRule>>;

        fn add_sys(map: &mut RulesMap, sys: i64) {
            let _rules = map.entry(sys).or_default();
        }

        fn add_sys_args_match(map: &mut RulesMap, sys: i64, rule: SeccompRule) {
            let rules = map.entry(sys).or_default();
            rules.push(rule);
        }

        let mut map: RulesMap = [].into();

        add_sys(&mut map, sys!(SYS_brk));
        add_sys(&mut map, sys!(SYS_close));
        #[cfg(not(target_os = "android"))]
        add_sys(&mut map, sys!(SYS_close_range));
        add_sys(&mut map, sys!(SYS_exit));
        add_sys(&mut map, sys!(SYS_exit_group));
        add_sys(&mut map, sys!(SYS_getpid));
        add_sys(&mut map, sys!(SYS_getrandom));
        add_sys(&mut map, sys!(SYS_gettid));
        add_sys(&mut map, sys!(SYS_madvise));
        add_sys(&mut map, sys!(SYS_munmap));
        add_sys(&mut map, sys!(SYS_sched_getaffinity));
        add_sys(&mut map, sys!(SYS_sigaltstack));
        add_sys(&mut map, sys!(SYS_nanosleep));
        add_sys(&mut map, sys!(SYS_clock_gettime));
        add_sys(&mut map, sys!(SYS_clock_getres));
        add_sys(&mut map, sys!(SYS_clock_nanosleep));
        add_sys(&mut map, sys!(SYS_gettimeofday));

        fn add_read_write_rules(map: &mut RulesMap) {
            add_sys(map, sys!(SYS_epoll_create1));
            add_sys(map, sys!(SYS_epoll_ctl));
            add_sys(map, sys!(SYS_epoll_pwait));
            #[cfg(all(any(target_arch = "x86_64", target_arch = "arm"), target_os = "linux"))]
            add_sys(map, sys!(SYS_epoll_pwait2));
            add_sys(map, sys!(SYS_epoll_wait));
            add_sys(map, sys!(SYS_lseek));
            add_sys(map, sys!(SYS_ppoll));
            add_sys(map, sys!(SYS_pselect6));
        }

        for allow in allow {
            match *allow {
                Allow::Mmap => {
                    #[cfg(any(
                        target_arch = "x86",
                        target_arch = "x86_64",
                        target_arch = "aarch64"
                    ))]
                    add_sys(&mut map, sys!(SYS_mmap));
                    #[cfg(any(target_arch = "x86", target_arch = "arm"))]
                    add_sys(&mut map, sys!(SYS_mmap2));
                    add_sys(&mut map, sys!(SYS_mremap));
                    add_sys(&mut map, sys!(SYS_munmap));
                }
                Allow::Mprotect => {
                    add_sys(&mut map, sys!(SYS_mprotect));
                }
                Allow::UnixConnect => {
                    add_sys(&mut map, sys!(SYS_connect));
                    add_sys_args_match(&mut map, sys!(SYS_socket), args!(0 == libc::AF_UNIX));
                    add_sys(&mut map, sys!(SYS_getsockopt));
                }
                Allow::UnixListen => {
                    add_sys(&mut map, sys!(SYS_accept4));
                    add_sys(&mut map, sys!(SYS_bind));
                    add_sys(&mut map, sys!(SYS_listen));
                    add_sys_args_match(&mut map, sys!(SYS_socket), args!(0 == libc::AF_INET));
                    add_sys_args_match(&mut map, sys!(SYS_socket), args!(0 == libc::AF_INET6));
                    add_sys(&mut map, sys!(SYS_getsockopt));
                }
                Allow::Open => {
                    //TODO: This should be restricted
                    add_sys(&mut map, sys!(SYS_open));
                    add_sys(&mut map, sys!(SYS_openat));
                }
                Allow::Read => {
                    add_sys(&mut map, sys!(SYS_pread64));
                    add_sys(&mut map, sys!(SYS_preadv2));
                    add_sys(&mut map, sys!(SYS_read));
                    add_sys(&mut map, sys!(SYS_readv));
                    add_read_write_rules(&mut map);
                }
                Allow::Write => {
                    add_sys(&mut map, sys!(SYS_fdatasync));
                    add_sys(&mut map, sys!(SYS_fsync));
                    add_sys(&mut map, sys!(SYS_pwrite64));
                    add_sys(&mut map, sys!(SYS_pwritev2));
                    add_sys(&mut map, sys!(SYS_write));
                    add_sys(&mut map, sys!(SYS_writev));
                    add_read_write_rules(&mut map);
                }
                Allow::Stat => {
                    add_sys(&mut map, sys!(SYS_fstat));
                    add_sys(&mut map, sys!(SYS_statx));
                    add_sys(&mut map, sys!(SYS_newfstatat));
                }
                Allow::Listdir => {
                    add_sys(&mut map, sys!(SYS_getdents64));
                }
                Allow::Recv => {
                    add_sys(&mut map, sys!(SYS_recvfrom));
                    add_sys(&mut map, sys!(SYS_recvmsg));
                    add_sys(&mut map, sys!(SYS_recvmmsg));
                }
                Allow::Send => {
                    add_sys(&mut map, sys!(SYS_sendto));
                    add_sys(&mut map, sys!(SYS_sendmsg));
                    add_sys(&mut map, sys!(SYS_sendmmsg));
                }
                Allow::Sendfile => {
                    add_sys(&mut map, sys!(SYS_sendfile));
                }
                Allow::Futex => {
                    add_sys(&mut map, sys!(SYS_futex));
                    add_sys(&mut map, sys!(SYS_get_robust_list));
                    add_sys(&mut map, sys!(SYS_set_robust_list));
                    #[cfg(all(
                        any(target_arch = "x86", target_arch = "x86_64", target_arch = "arm"),
                        target_os = "linux"
                    ))]
                    add_sys(&mut map, sys!(SYS_futex_waitv));
                    //add_sys(&mut map, sys!(SYS_futex_wake));
                    //add_sys(&mut map, sys!(SYS_futex_wait));
                    //add_sys(&mut map, sys!(SYS_futex_requeue));
                }
                Allow::Signal => {
                    add_sys(&mut map, sys!(SYS_rt_sigreturn));
                    add_sys(&mut map, sys!(SYS_rt_sigprocmask));
                }
                Allow::Threading => {
                    add_sys(&mut map, sys!(SYS_clone3)); //TODO restrict to threads
                    add_sys(&mut map, sys!(SYS_rseq));
                }
                Allow::Inotify => {
                    add_sys(&mut map, sys!(SYS_inotify_init));
                    add_sys(&mut map, sys!(SYS_inotify_add_watch));
                    add_sys(&mut map, sys!(SYS_inotify_rm_watch));
                }
                Allow::Prctl => {
                    //TODO: This should be restricted
                    add_sys(&mut map, sys!(SYS_prctl));
                }
                Allow::Timer => {
                    add_sys(&mut map, sys!(SYS_timer_create));
                    add_sys(&mut map, sys!(SYS_timer_settime));
                    add_sys(&mut map, sys!(SYS_timer_gettime));
                    add_sys(&mut map, sys!(SYS_timer_getoverrun));
                    add_sys(&mut map, sys!(SYS_timer_delete));
                }
                Allow::ClockGet => {
                    add_sys(&mut map, sys!(SYS_clock_gettime));
                    add_sys(&mut map, sys!(SYS_clock_getres));
                }
                Allow::ClockSet => {
                    add_sys(&mut map, sys!(SYS_clock_settime));
                }
                Allow::Sleep => {
                    add_sys(&mut map, sys!(SYS_nanosleep));
                    add_sys(&mut map, sys!(SYS_clock_nanosleep));
                }
            }
        }

        let filter = SeccompFilter::new(
            map,
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
