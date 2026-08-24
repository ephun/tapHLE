/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `clocale.h`

use std::collections::hash_map::Entry;

use crate::dyld::{ConstantExports, FunctionExports, HostConstant};
use crate::environment::Environment;
use crate::export_c_func;
use crate::mem::{ConstPtr, ConstVoidPtr, MutPtr};

pub type LocaleCategory = i32;
pub const LC_ALL: LocaleCategory = 0;
pub const LC_COLLATE: LocaleCategory = 1;
pub const LC_CTYPE: LocaleCategory = 2;
pub const LC_MONETARY: LocaleCategory = 3;
pub const LC_NUMERIC: LocaleCategory = 4;
pub const LC_TIME: LocaleCategory = 5;
pub const LC_MESSAGES: LocaleCategory = 6;

#[derive(Default)]
pub struct State {
    locale: std::collections::HashMap<LocaleCategory, MutPtr<u8>>,
}

pub fn setlocale(
    env: &mut Environment,
    category: LocaleCategory,
    locale: ConstPtr<u8>,
) -> MutPtr<u8> {
    assert!(matches!(
        category,
        LC_ALL | LC_COLLATE | LC_CTYPE | LC_MONETARY | LC_NUMERIC | LC_TIME | LC_MESSAGES
    ));
    if !locale.is_null() {
        // TODO: Handle empty locale string and ensure the combination of
        // category and locale is valid.
        let locale_cstr = env.mem.cstr_at(locale).to_owned();
        assert_ne!(locale_cstr.len(), 0);
        let new_locale = env.mem.alloc_and_write_cstr(locale_cstr.as_slice());
        if let Some(old_locale) = env.libc_state.clocale.locale.insert(category, new_locale) {
            env.mem.free(old_locale.cast())
        };
    } else if let Entry::Vacant(entry) = env.libc_state.clocale.locale.entry(category) {
        let default_locale = env.mem.alloc_and_write_cstr(b"C");
        entry.insert(default_locale);
    }
    env.libc_state.clocale.locale.get(&category).unwrap().cast()
}

/// `MB_CUR_MAX` is a macro for this variable, and it is how any code that walks
/// a string byte by byte decides whether a byte can be a character on its own.
/// It is 1 in the "C" locale, which is the only locale tapHLE describes:
/// `setlocale()` above hands back "C", and the rune locale in `ctype.rs`
/// declares its encoding as "NONE".
///
/// It is exported because an unbound data import is a null pointer sitting in
/// `__DATA` waiting to be read, not a harmless warning — and this one is
/// referenced by 40 of the 50 apps in the local collection, more than any other
/// unbound symbol. Whatever else those apps do, none of them should be
/// dereferencing null to ask how wide a character is.
fn mb_cur_max(env: &mut Environment) -> ConstVoidPtr {
    env.mem.alloc_and_write(1i32).cast_void().cast_const()
}

pub const CONSTANTS: ConstantExports = &[("___mb_cur_max", HostConstant::Custom(mb_cur_max))];

pub const FUNCTIONS: FunctionExports = &[export_c_func!(setlocale(_, _))];
