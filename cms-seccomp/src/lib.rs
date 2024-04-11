// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 2 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

#![forbid(unsafe_code)]

use anyhow::{self as ah, Context as _};
use seccompiler::{apply_filter_all_threads, BpfProgram, SeccompAction, SeccompFilter};
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
    Recv,
    Send,
    Sendfile,
    Futex,
    Signal,
    SignalMask,
    SignalReturn,
    Threading,
}

#[derive(Clone, Debug)]
pub enum Action {
    Kill,
    Log,
}

pub struct Filter(BpfProgram);

pub fn seccomp_compile(allow: &[Allow], deny_action: Action) -> ah::Result<Filter> {
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
        rules.insert(libc::SYS_epoll_create, vec![]);
        rules.insert(libc::SYS_epoll_create1, vec![]);
        rules.insert(libc::SYS_epoll_ctl, vec![]);
        rules.insert(libc::SYS_epoll_pwait, vec![]);
        rules.insert(libc::SYS_epoll_pwait2, vec![]);
        rules.insert(libc::SYS_epoll_wait, vec![]);
        rules.insert(libc::SYS_poll, vec![]);
        rules.insert(libc::SYS_ppoll, vec![]);
        rules.insert(libc::SYS_pselect6, vec![]);
        rules.insert(libc::SYS_select, vec![]);
        rules.insert(libc::SYS_lseek, vec![]);
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
                rules.insert(libc::SYS_pwritev, vec![]);
                rules.insert(libc::SYS_pwritev2, vec![]);
                rules.insert(libc::SYS_write, vec![]);
                rules.insert(libc::SYS_writev, vec![]);
                add_read_write_rules(&mut rules);
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
                rules.insert(libc::SYS_clone, vec![]); //TODO restrict to threads
                rules.insert(libc::SYS_clone3, vec![]); //TODO restrict to threads
                rules.insert(libc::SYS_rseq, vec![]);
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
        ARCH.try_into().context("Unsupported CPU ARCH")?,
    )
    .context("Create seccomp filter")?;

    let filter: BpfProgram = filter.try_into().context("Seccomp to BPF")?;

    Ok(Filter(filter))
}

pub fn seccomp_install(filter: Filter) -> ah::Result<()> {
    apply_filter_all_threads(&filter.0).context("Apply seccomp filter")
}

// vim: ts=4 sw=4 expandtab
