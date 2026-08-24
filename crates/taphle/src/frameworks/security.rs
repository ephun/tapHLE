/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Security framework: the keychain item API.
//!
//! Only generic passwords are modelled, which is what SDKs embedded in games
//! use the keychain for — a device or session token they want to survive being
//! read back later in the same session.
//!
//! The store is **in memory and per run**: nothing is written to disk. A game's
//! keychain use is normally an SDK caching a token it can re-fetch, so losing
//! it between runs looks like a fresh install, which those SDKs handle. Writing
//! it out would mean persisting third-party credentials on the host, which is
//! not something tapHLE should do without a deliberate decision.
//!
//! Resources:
//! - Apple's [Keychain Services](https://developer.apple.com/documentation/security/keychain_services)

use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::frameworks::foundation::ns_string::to_rust_string;
use crate::mem::MutPtr;
use crate::objc::{id, msg, nil, release, retain};
use crate::Environment;

/// `OSStatus`.
type OSStatus = i32;

const errSecSuccess: OSStatus = 0;
const errSecItemNotFound: OSStatus = -25300;
const errSecDuplicateItem: OSStatus = -25299;
const errSecParam: OSStatus = -50;

/// One stored generic-password item.
struct KeychainItem {
    service: String,
    account: String,
    /// `NSData*`, retained for as long as the item exists.
    data: id,
}

#[derive(Default)]
pub struct State {
    items: Vec<KeychainItem>,
}

/// Read a string-valued key out of a query/attributes dictionary.
fn string_for_key(env: &mut Environment, dict: id, key: &'static str) -> Option<String> {
    let key = crate::frameworks::foundation::ns_string::get_static_str(env, key);
    let value: id = msg![env; dict objectForKey:key];
    if value == nil {
        None
    } else {
        Some(to_rust_string(env, value).to_string())
    }
}

fn object_for_key(env: &mut Environment, dict: id, key: &'static str) -> id {
    let key = crate::frameworks::foundation::ns_string::get_static_str(env, key);
    msg![env; dict objectForKey:key]
}

/// Find the index of the item a query selects, if any.
///
/// Matching is by service + account, the pair that identifies a generic
/// password. A query naming neither matches the first item, which is what
/// `kSecMatchLimitOne` with no further constraint asks for.
fn find_item(env: &mut Environment, query: id) -> Option<usize> {
    let service = string_for_key(env, query, kSecAttrService);
    let account = string_for_key(env, query, kSecAttrAccount);
    env.framework_state.security.items.iter().position(|item| {
        service.as_ref().is_none_or(|s| *s == item.service)
            && account.as_ref().is_none_or(|a| *a == item.account)
    })
}

fn SecItemAdd(env: &mut Environment, attributes: id, result: MutPtr<id>) -> OSStatus {
    if attributes == nil {
        return errSecParam;
    }
    if find_item(env, attributes).is_some() {
        return errSecDuplicateItem;
    }

    let service = string_for_key(env, attributes, kSecAttrService).unwrap_or_default();
    let account = string_for_key(env, attributes, kSecAttrAccount).unwrap_or_default();
    let data = object_for_key(env, attributes, kSecValueData);
    retain(env, data);
    env.framework_state.security.items.push(KeychainItem {
        service,
        account,
        data,
    });

    if !result.is_null() {
        // The caller owns anything returned here, so hand back a retained
        // reference rather than the one the store holds.
        retain(env, data);
        env.mem.write(result, data);
    }
    errSecSuccess
}

fn SecItemCopyMatching(env: &mut Environment, query: id, result: MutPtr<id>) -> OSStatus {
    if query == nil {
        return errSecParam;
    }
    let Some(index) = find_item(env, query) else {
        // The usual answer on a fresh run, and the one SDKs are written to
        // handle: no saved item, so make a new one.
        return errSecItemNotFound;
    };
    if result.is_null() {
        return errSecSuccess;
    }

    let data = env.framework_state.security.items[index].data;
    // Only the data form is modelled. Returning the data for an attributes
    // request would be worse than saying nothing, so answer "not found" and let
    // the caller re-create the item.
    let return_data: id = object_for_key(env, query, kSecReturnData);
    let wants_data: bool = return_data != nil && msg![env; return_data boolValue];
    if !wants_data {
        log!(
            "TODO: SecItemCopyMatching() only returns {}; \
             attribute and persistent-ref requests answer errSecItemNotFound",
            kSecValueData
        );
        return errSecItemNotFound;
    }

    retain(env, data);
    env.mem.write(result, data);
    errSecSuccess
}

fn SecItemUpdate(env: &mut Environment, query: id, attributes_to_update: id) -> OSStatus {
    if query == nil || attributes_to_update == nil {
        return errSecParam;
    }
    let Some(index) = find_item(env, query) else {
        return errSecItemNotFound;
    };
    let new_data = object_for_key(env, attributes_to_update, kSecValueData);
    if new_data == nil {
        // Nothing this implementation stores was asked to change.
        return errSecSuccess;
    }
    retain(env, new_data);
    let old_data = std::mem::replace(
        &mut env.framework_state.security.items[index].data,
        new_data,
    );
    release(env, old_data);
    errSecSuccess
}

fn SecItemDelete(env: &mut Environment, query: id) -> OSStatus {
    if query == nil {
        return errSecParam;
    }
    let Some(index) = find_item(env, query) else {
        return errSecItemNotFound;
    };
    let item = env.framework_state.security.items.remove(index);
    release(env, item.data);
    errSecSuccess
}

// The documented CFString values of the keychain query keys. Apps compare and
// hash these, and several arrive as unhandled non-lazy symbols that crash on
// dereference when left null.
pub const kSecClass: &str = "class";
pub const kSecClassGenericPassword: &str = "genp";
pub const kSecClassInternetPassword: &str = "inet";
pub const kSecAttrService: &str = "svce";
pub const kSecAttrAccount: &str = "acct";
pub const kSecAttrGeneric: &str = "gena";
pub const kSecAttrLabel: &str = "labl";
pub const kSecAttrDescription: &str = "desc";
pub const kSecAttrAccessible: &str = "pdmn";
pub const kSecValueData: &str = "v_Data";
pub const kSecReturnData: &str = "r_Data";
pub const kSecReturnAttributes: &str = "r_Attributes";
pub const kSecReturnPersistentRef: &str = "r_PersistentRef";
pub const kSecMatchLimit: &str = "m_Limit";
pub const kSecMatchLimitOne: &str = "m_LimitOne";
pub const kSecMatchLimitAll: &str = "m_LimitAll";
pub const kSecMatchItemList: &str = "m_ItemList";

pub const CONSTANTS: ConstantExports = &[
    ("_kSecClass", HostConstant::NSString(kSecClass)),
    (
        "_kSecClassGenericPassword",
        HostConstant::NSString(kSecClassGenericPassword),
    ),
    (
        "_kSecClassInternetPassword",
        HostConstant::NSString(kSecClassInternetPassword),
    ),
    ("_kSecAttrService", HostConstant::NSString(kSecAttrService)),
    ("_kSecAttrAccount", HostConstant::NSString(kSecAttrAccount)),
    ("_kSecAttrGeneric", HostConstant::NSString(kSecAttrGeneric)),
    ("_kSecAttrLabel", HostConstant::NSString(kSecAttrLabel)),
    (
        "_kSecAttrDescription",
        HostConstant::NSString(kSecAttrDescription),
    ),
    (
        "_kSecAttrAccessible",
        HostConstant::NSString(kSecAttrAccessible),
    ),
    ("_kSecValueData", HostConstant::NSString(kSecValueData)),
    ("_kSecReturnData", HostConstant::NSString(kSecReturnData)),
    (
        "_kSecReturnAttributes",
        HostConstant::NSString(kSecReturnAttributes),
    ),
    (
        "_kSecReturnPersistentRef",
        HostConstant::NSString(kSecReturnPersistentRef),
    ),
    ("_kSecMatchLimit", HostConstant::NSString(kSecMatchLimit)),
    (
        "_kSecMatchLimitOne",
        HostConstant::NSString(kSecMatchLimitOne),
    ),
    (
        "_kSecMatchLimitAll",
        HostConstant::NSString(kSecMatchLimitAll),
    ),
    (
        "_kSecMatchItemList",
        HostConstant::NSString(kSecMatchItemList),
    ),
];

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(SecItemAdd(_, _)),
    export_c_func!(SecItemCopyMatching(_, _)),
    export_c_func!(SecItemUpdate(_, _)),
    export_c_func!(SecItemDelete(_)),
];

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/Security.framework/Security",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[CONSTANTS],
    function_exports: &[FUNCTIONS],
};
