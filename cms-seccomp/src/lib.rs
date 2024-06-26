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
    SignalMask,
    SignalReturn,
    Threading,
    Inotify,
    Prctl,
    Timer,
    ClockGet,
    ClockSet,
    Sleep,
}

#[derive(Clone, Debug)]
pub enum Action {
    Kill,
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
}

pub fn seccomp_compile(allow: &[Allow], deny_action: Action) -> ah::Result<Filter> {
    seccomp_compile_for_arch(allow, deny_action, ARCH)
}

pub fn seccomp_compile_for_arch(
    allow: &[Allow],
    deny_action: Action,
    arch: &str,
) -> ah::Result<Filter> {
    let mut rules: BTreeMap<_, _> = [
        (libc::SYS_brk, vec![]),
        (libc::SYS_close, vec![]),
        (libc::SYS_close_range, vec![]),
        (libc::SYS_exit, vec![]),
        (libc::SYS_exit_group, vec![]),
        (libc::SYS_getpid, vec![]),
        (libc::SYS_getrandom, vec![]),
        (libc::SYS_gettid, vec![]),
        (libc::SYS_madvise, vec![]),
        (libc::SYS_munmap, vec![]),
        (libc::SYS_sched_getaffinity, vec![]),
        (libc::SYS_sigaltstack, vec![]),
    ]
    .into();

    let add_read_write_rules = |rules: &mut BTreeMap<_, _>| {
        #[cfg(feature = "oldsyscalls")]
        rules.insert(libc::SYS_epoll_create, vec![]);
        rules.insert(libc::SYS_epoll_create1, vec![]);
        rules.insert(libc::SYS_epoll_ctl, vec![]);
        #[cfg(feature = "oldsyscalls")]
        rules.insert(libc::SYS_epoll_pwait, vec![]);
        rules.insert(libc::SYS_epoll_pwait2, vec![]);
        rules.insert(libc::SYS_epoll_wait, vec![]);
        rules.insert(libc::SYS_lseek, vec![]);
        #[cfg(feature = "oldsyscalls")]
        rules.insert(libc::SYS_poll, vec![]);
        rules.insert(libc::SYS_ppoll, vec![]);
        rules.insert(libc::SYS_pselect6, vec![]);
        #[cfg(feature = "oldsyscalls")]
        rules.insert(libc::SYS_select, vec![]);
    };

    for allow in allow {
        match *allow {
            Allow::Mmap => {
                rules.insert(libc::SYS_mmap, vec![]);
                rules.insert(libc::SYS_mremap, vec![]);
                rules.insert(libc::SYS_munmap, vec![]);
            }
            Allow::Mprotect => {
                rules.insert(libc::SYS_mprotect, vec![]);
            }
            Allow::UnixConnect => {
                rules.insert(libc::SYS_connect, vec![]);
                rules.insert(libc::SYS_socket, vec![]); //TODO: Restrict to AF_UNIX
            }
            Allow::UnixListen => {
                #[cfg(feature = "oldsyscalls")]
                rules.insert(libc::SYS_accept, vec![]);
                rules.insert(libc::SYS_accept4, vec![]);
                rules.insert(libc::SYS_bind, vec![]);
                rules.insert(libc::SYS_listen, vec![]);
                rules.insert(libc::SYS_socket, vec![]); //TODO: Restrict to AF_UNIX
            }
            Allow::Open => {
                //TODO: This should be restricted
                rules.insert(libc::SYS_open, vec![]);
                rules.insert(libc::SYS_openat, vec![]);
            }
            Allow::Read => {
                rules.insert(libc::SYS_pread64, vec![]);
                #[cfg(feature = "oldsyscalls")]
                rules.insert(libc::SYS_preadv, vec![]);
                rules.insert(libc::SYS_preadv2, vec![]);
                rules.insert(libc::SYS_read, vec![]);
                rules.insert(libc::SYS_readv, vec![]);
                add_read_write_rules(&mut rules);
            }
            Allow::Write => {
                rules.insert(libc::SYS_fdatasync, vec![]);
                rules.insert(libc::SYS_fsync, vec![]);
                rules.insert(libc::SYS_pwrite64, vec![]);
                #[cfg(feature = "oldsyscalls")]
                rules.insert(libc::SYS_pwritev, vec![]);
                rules.insert(libc::SYS_pwritev2, vec![]);
                rules.insert(libc::SYS_write, vec![]);
                rules.insert(libc::SYS_writev, vec![]);
                add_read_write_rules(&mut rules);
            }
            Allow::Stat => {
                #[cfg(feature = "oldsyscalls")]
                rules.insert(libc::SYS_stat, vec![]);
                #[cfg(feature = "oldsyscalls")]
                rules.insert(libc::SYS_fstat, vec![]);
                #[cfg(feature = "oldsyscalls")]
                rules.insert(libc::SYS_lstat, vec![]);
                rules.insert(libc::SYS_statx, vec![]);
                rules.insert(libc::SYS_newfstatat, vec![]);
            }
            Allow::Listdir => {
                #[cfg(feature = "oldsyscalls")]
                rules.insert(libc::SYS_getdents, vec![]);
                rules.insert(libc::SYS_getdents64, vec![]);
            }
            Allow::Recv => {
                rules.insert(libc::SYS_recvfrom, vec![]);
                rules.insert(libc::SYS_recvmsg, vec![]);
                rules.insert(libc::SYS_recvmmsg, vec![]);
            }
            Allow::Send => {
                rules.insert(libc::SYS_sendto, vec![]);
                rules.insert(libc::SYS_sendmsg, vec![]);
                rules.insert(libc::SYS_sendmmsg, vec![]);
            }
            Allow::Sendfile => {
                rules.insert(libc::SYS_sendfile, vec![]);
            }
            Allow::Futex => {
                rules.insert(libc::SYS_futex, vec![]);
                rules.insert(libc::SYS_get_robust_list, vec![]);
                rules.insert(libc::SYS_set_robust_list, vec![]);
            }
            Allow::Signal => {
                rules.insert(libc::SYS_rt_sigaction, vec![]);
                rules.insert(libc::SYS_rt_sigprocmask, vec![]);
            }
            Allow::SignalMask => {
                rules.insert(libc::SYS_rt_sigprocmask, vec![]);
            }
            Allow::SignalReturn => {
                rules.insert(libc::SYS_rt_sigreturn, vec![]);
            }
            Allow::Threading => {
                #[cfg(feature = "oldsyscalls")]
                rules.insert(libc::SYS_clone, vec![]); //TODO restrict to threads
                rules.insert(libc::SYS_clone3, vec![]); //TODO restrict to threads
                rules.insert(libc::SYS_rseq, vec![]);
            }
            Allow::Inotify => {
                rules.insert(libc::SYS_inotify_init, vec![]);
                rules.insert(libc::SYS_inotify_add_watch, vec![]);
                rules.insert(libc::SYS_inotify_rm_watch, vec![]);
            }
            Allow::Prctl => {
                //TODO: This should be restricted
                rules.insert(libc::SYS_prctl, vec![]);
            }
            Allow::Timer => {
                rules.insert(libc::SYS_timer_create, vec![]);
                rules.insert(libc::SYS_timer_settime, vec![]);
                rules.insert(libc::SYS_timer_gettime, vec![]);
                rules.insert(libc::SYS_timer_getoverrun, vec![]);
                rules.insert(libc::SYS_timer_delete, vec![]);
            }
            Allow::ClockGet => {
                rules.insert(libc::SYS_clock_gettime, vec![]);
                rules.insert(libc::SYS_clock_getres, vec![]);
            }
            Allow::ClockSet => {
                rules.insert(libc::SYS_clock_settime, vec![]);
            }
            Allow::Sleep => {
                rules.insert(libc::SYS_nanosleep, vec![]);
                rules.insert(libc::SYS_clock_nanosleep, vec![]);
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

pub fn seccomp_install(filter: Filter) -> ah::Result<()> {
    apply_filter_all_threads(&filter.0).context("Apply seccomp filter")
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_filter_serialize() {
        let filter = seccomp_compile(&[Allow::Read], Action::Kill).unwrap();
        let filter2 = Filter::deserialize(&filter.serialize());
        assert_eq!(filter.0.len(), filter2.0.len());
        for i in 0..filter.0.len() {
            assert_eq!(filter.0[i], filter2.0[i]);
        }
    }
}

// vim: ts=4 sw=4 expandtab
