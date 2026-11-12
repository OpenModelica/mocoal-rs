// This file is part of mocoal-rs.
// Copyright (c) 2026 Open Source Modelica Consortium (OSMC)
//
// This program is provided under the terms of the OSMC Public License (OSMC-PL) version 1.8 or
// the GNU AGPL version 3 license, according to the recipient's choice. See LICENSE.md.

//! A thread with a large stack for the layout. The parser descends recursively: each level of
//! nesting in a model (parentheses, packages) takes about 3 KB of stack in a release build, so an
//! 8 MB main thread overflows at about 2500 levels, which omc still loads (it stops at 10000). A
//! stack overflow aborts the process; no `catch_unwind` stops it.

use std::any::Any;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;
use std::thread;

/// 256 MB: about 80000 levels. Only the part that is used takes memory. `--stack MB` sets another.
pub const STACK_MB: usize = 256;

/// The largest `--stack`: 64 GB.
pub const MAX_STACK_MB: usize = 64 << 10;

/// `f()` on a thread with a stack of `mb` MB. `Err` when `f` panics. Without such a thread
/// (a limit on memory), `f()` runs on the current thread.
pub fn on_large_stack<T: Send>(mb: usize, f: impl FnOnce() -> T + Send) -> Result<T, Box<dyn Any + Send>> {
    // The thread takes `f` from here; when there is no thread, `f` is still here.
    let slot = Mutex::new(Some(f));
    let take = || slot.lock().unwrap_or_else(|e| e.into_inner()).take();
    let spawned = thread::scope(|s| {
        if no_thread() {
            return Err(io::Error::other("no thread (test)"));
        }
        thread::Builder::new()
            .stack_size(mb << 20)
            .spawn_scoped(s, || take().map(|f| f()))
            .map(|handle| handle.join())
    });
    match spawned {
        Ok(Ok(Some(value))) => Ok(value),
        Ok(Ok(None)) => unreachable!("the thread runs f"),
        Ok(Err(panic)) => Err(panic),
        Err(_) => match take() {
            Some(f) => catch_unwind(AssertUnwindSafe(f)),
            None => unreachable!("no thread took f"),
        },
    }
}

/// True when a test asks for no thread, as under a limit on memory.
#[cfg(not(test))]
fn no_thread() -> bool {
    false
}

#[cfg(test)]
fn no_thread() -> bool {
    tests::NO_THREAD.get()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    thread_local! {
        /// Set by a test: `on_large_stack` gets no thread.
        pub static NO_THREAD: Cell<bool> = const { Cell::new(false) };
    }
    use crate::api::layout_string;
    use crate::options::Options;

    /// A model whose equation has `n` nested parentheses.
    fn deep(n: usize) -> String {
        format!(
            "model Deep\n  Real x = {}1{};\nend Deep;\n",
            "(".repeat(n),
            ")".repeat(n)
        )
    }

    #[test]
    fn deep_nesting_on_the_large_stack() {
        // 4000 levels overflow the 8 MB main thread of a release build and the 2 MB test thread.
        let text = deep(4000);
        let result = on_large_stack(STACK_MB, || layout_string(&text, "Deep.mo", &Options::default()));
        assert!(matches!(result, Ok(Ok(_))));
    }

    #[test]
    fn without_the_thread_on_the_current_one() {
        NO_THREAD.set(true);
        let value = on_large_stack(STACK_MB, || 7).ok();
        let panic = on_large_stack(STACK_MB, || -> u8 { panic!("test") }).is_err();
        NO_THREAD.set(false);
        assert_eq!(value, Some(7));
        assert!(panic);
    }

    #[test]
    fn a_panic_is_an_error() {
        assert!(on_large_stack(STACK_MB, || -> u8 { panic!("test") }).is_err());
        assert_eq!(on_large_stack(STACK_MB, || 7).ok(), Some(7));
    }
}
