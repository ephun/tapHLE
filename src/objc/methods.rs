/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Handling of Objective-C methods.
//!
//! Resources:
//! - [Apple's documentation of `class_addMethod`](https://developer.apple.com/documentation/objectivec/1418901-class_addmethod?language=objc)

use super::{
    id, nil, objc_super, Class, ClassHostObject, MsgSendSignature, MsgSendSuperSignature, ObjC, SEL,
};
use crate::abi::{CallFromGuest, DotDotDot, GuestArg, GuestFunction, GuestRet};
use crate::dyld::HostFunction;
use crate::mem::{guest_size_of, ConstPtr, GuestUSize, Mem, MutPtr, MutVoidPtr, Ptr, SafeRead};
use crate::Environment;
use std::any::TypeId;

/// Type for any function implementating a method.
///
/// The name is standard Objective-C.
///
/// In our implementation, we have both "host methods" (Rust functions) and
/// "guest methods" (functions in the guest app). Either way, the function needs
/// to conform to the same ABI: [id] and [SEL] must be its first two parameters.
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy)]
pub enum IMP {
    Host(&'static dyn HostIMP),
    Guest(GuestIMP),
}

/// Type for any host function implementing a method (see also [IMP]).
pub trait HostIMP: CallFromGuest {
    /// See [MsgSendSignature::type_info].
    fn type_info(&self) -> (TypeId, &'static str);
}

macro_rules! impl_HostIMP {
    ( $($P:ident),* ) => {
        impl<R, $($P,)*> HostIMP for fn(&mut Environment, id, SEL, $($P,)*) -> R
        where
            R: GuestRet + 'static,
            $($P: GuestArg + 'static,)*
        {
            fn type_info(&self) -> (TypeId, &'static str) {
                <(R, (id, SEL, $($P,)*)) as MsgSendSignature>::type_info()
            }
        }
        impl<R, $($P,)*> HostIMP for fn(&mut Environment, id, SEL, $($P,)* DotDotDot) -> R
        where
            R: GuestRet + 'static,
            $($P: GuestArg + 'static,)*
        {
            fn type_info(&self) -> (TypeId, &'static str) {
                todo!("host-to-host message calls with var-args"); // TODO
            }
        }

        // Currently there is a one-to-one mapping between valid host IMP
        // parameters and valid host message send arguments, so the traits for
        // the latter are also implemented here for convenience.

        impl<R, $($P,)*> MsgSendSignature for (R, (id, SEL, $($P,)*))
        where
            R: GuestRet + 'static,
            $($P: GuestArg + 'static,)*
        {
        }
        impl<R, $($P,)*> MsgSendSuperSignature for (R, (ConstPtr<objc_super>, SEL, $($P,)*))
        where
            R: GuestRet + 'static,
            $($P: GuestArg + 'static,)*
        {
            type WithoutSuper = (R, (id, SEL, $($P,)*));
        }
    }
}

impl_HostIMP!();
impl_HostIMP!(P1);
impl_HostIMP!(P1, P2);
impl_HostIMP!(P1, P2, P3);
impl_HostIMP!(P1, P2, P3, P4);
impl_HostIMP!(P1, P2, P3, P4, P5);
// Six is not arbitrary: -[NSTimer initWithFireDate:interval:target:selector:
// userInfo:repeats:] needs it, and it is the longest selector any host class
// implements. CallFromGuest goes further, so raising this again is one line.
impl_HostIMP!(P1, P2, P3, P4, P5, P6);

/// Type for a guest function implementing a method. See [GuestFunction].
pub type GuestIMP = GuestFunction;

/// The layout of a method list in an app binary.
///
/// The name, field names and field layout are based on what Ghidra outputs.
#[repr(C, packed)]
pub(super) struct method_list_t {
    entsize: GuestUSize,
    count: GuestUSize,
    // entries follow the struct
}
unsafe impl SafeRead for method_list_t {}

/// The layout of a method in an app binary.
///
/// The name, field names and field layout are based on what Ghidra outputs.
#[repr(C, packed)]
struct method_t {
    name: ConstPtr<u8>,
    types: ConstPtr<u8>,
    imp: GuestIMP,
}
unsafe impl SafeRead for method_t {}

impl ClassHostObject {
    // See classes.rs for host method parsing

    pub(super) fn add_methods_from_bin(
        &mut self,
        method_list_ptr: ConstPtr<method_list_t>,
        mem: &Mem,
        objc: &mut ObjC,
    ) {
        let method_list_t { entsize, count } = mem.read(method_list_ptr);
        assert!(entsize >= guest_size_of::<method_t>());

        let methods_base_ptr: ConstPtr<method_t> = (method_list_ptr + 1).cast();

        for i in 0..count {
            let method_ptr: ConstPtr<method_t> =
                Ptr::from_bits(methods_base_ptr.to_bits() + i * entsize);

            let method_t { name, types, imp } = mem.read(method_ptr);

            // There is no guarantee this string is unique or known.
            // We must deduplicate it like any other.
            let sel = objc.register_bin_selector(name, mem);
            self.methods.insert(sel, IMP::Guest(imp));
            // TODO: avoid storing duplicated signatures globally
            self.guest_method_signatures.insert(sel, types);
        }
    }
}

impl ObjC {
    /// Checks if the provided class has a method in its class chain (that is
    /// to say, objects of the given class respond to a selector).
    pub fn class_has_method(&self, class: Class, sel: SEL) -> bool {
        let mut class = class;
        loop {
            let &ClassHostObject {
                superclass,
                ref methods,
                ..
            } = self.borrow(class);
            if methods.contains_key(&sel) {
                return true;
            } else if superclass == nil {
                return false;
            } else {
                class = superclass;
            }
        }
    }

    /// Returns the implementation selected by normal Objective-C inheritance
    /// lookup, if the class responds to the selector.
    pub fn class_get_method_implementation(&self, class: Class, sel: SEL) -> Option<IMP> {
        let mut class = class;
        loop {
            let &ClassHostObject {
                superclass,
                ref methods,
                ..
            } = self.borrow(class);
            if let Some(&imp) = methods.get(&sel) {
                return Some(imp);
            } else if superclass == nil {
                return None;
            } else {
                class = superclass;
            }
        }
    }

    /// Variant of `class_has_method` which doesn't account for inheritance.
    pub fn class_has_uninherited_method(&self, class: Class, sel: SEL) -> bool {
        let ClassHostObject { methods, .. } = self.borrow(class);
        methods.contains_key(&sel)
    }

    pub fn class_get_method_signature(&self, class: Class, sel: SEL) -> Option<&ConstPtr<u8>> {
        // TODO: support `host` method signatures
        let mut class = class;
        loop {
            let &ClassHostObject {
                superclass,
                ref methods,
                ref guest_method_signatures,
                ..
            } = self.borrow(class);
            if methods.contains_key(&sel) {
                return guest_method_signatures.get(&sel);
            } else if superclass == nil {
                return None;
            } else {
                class = superclass;
            }
        }
    }

    /// Same as [Self::class_has_method], but using a named selector (rather
    /// than a pointer).
    pub fn class_has_method_named(&self, class: Class, sel_name: &str) -> bool {
        if let Some(sel) = self.lookup_selector(sel_name) {
            self.class_has_method(class, sel)
        } else {
            false
        }
    }

    /// Checks if a given object has a method (responds to a selector).
    pub fn object_has_method(&self, mem: &Mem, obj: id, sel: SEL) -> bool {
        self.class_has_method(ObjC::read_isa(obj, mem), sel)
    }

    /// Returns the implementation selected by normal Objective-C inheritance
    /// lookup for an object, if it responds to the selector.
    pub fn object_get_method_implementation(&self, mem: &Mem, obj: id, sel: SEL) -> Option<IMP> {
        self.class_get_method_implementation(ObjC::read_isa(obj, mem), sel)
    }

    /// Variant of `object_has_method` which doesn't account for inheritance.
    pub fn object_has_uninherited_method(&self, mem: &Mem, obj: id, sel: SEL) -> bool {
        self.class_has_uninherited_method(ObjC::read_isa(obj, mem), sel)
    }

    #[allow(dead_code)]
    pub fn object_get_method_signature(
        &self,
        mem: &Mem,
        obj: id,
        sel: SEL,
    ) -> Option<&ConstPtr<u8>> {
        self.class_get_method_signature(ObjC::read_isa(obj, mem), sel)
    }

    /// Same as [Self::object_has_method], but using a named selector (rather
    /// than a pointer).
    pub fn object_has_method_named(&self, mem: &Mem, obj: id, sel_name: &str) -> bool {
        if let Some(sel) = self.lookup_selector(sel_name) {
            self.object_has_method(mem, obj, sel)
        } else {
            false
        }
    }

    /// Checks if a class overrides a method provided by its superclass.
    ///
    /// This looks through a superclass chain looking for the selector, stopping
    /// when the superclass is hit (and panicking if it never is). It does not
    /// check whether the selector is actually a method on the superclass.
    pub fn class_overrides_method_of_superclass(
        &self,
        class: Class,
        sel: SEL,
        superclass: Class,
    ) -> bool {
        let mut class = class;
        loop {
            if class == superclass {
                return false;
            }

            let &ClassHostObject {
                superclass,
                ref methods,
                ..
            } = self.borrow(class);
            if methods.contains_key(&sel) {
                return true;
            } else if superclass == nil {
                panic!();
            } else {
                class = superclass;
            }
        }
    }

    pub fn debug_all_class_selectors_as_strings(&self, mem: &Mem, class: Class) -> Vec<String> {
        let mut class = class;
        let mut selector_strings = Vec::new();
        loop {
            let &ClassHostObject {
                superclass,
                ref methods,
                ..
            } = self.borrow(class);
            let mut class_selector_strings = methods
                .keys()
                .map(|sel| sel.as_str(mem).to_string())
                .collect();
            selector_strings.append(&mut class_selector_strings);
            if superclass == nil {
                break;
            } else {
                class = superclass;
            }
        }
        selector_strings
    }

    /// Borrow a class's host object as a [ClassHostObject], returning [None]
    /// for `nil`, unknown, unimplemented, or faked classes (whose method tables
    /// we cannot edit).
    fn class_host_object(&self, class: Class) -> Option<&ClassHostObject> {
        if class == nil {
            return None;
        }
        self.get_host_object(class)?
            .as_any()
            .downcast_ref::<ClassHostObject>()
    }

    /// Find the class in `class`'s chain that actually defines `sel`, i.e. the
    /// class whose own method table holds it. Mirrors what the real runtime's
    /// `class_getInstanceMethod` returns a `Method` from.
    pub(super) fn class_defining_method(&self, class: Class, sel: SEL) -> Option<Class> {
        let mut class = class;
        loop {
            let host = self.class_host_object(class)?;
            if host.methods.contains_key(&sel) {
                return Some(class);
            }
            class = host.superclass;
            if class == nil {
                return None;
            }
        }
    }
}

/// Guest-visible layout of an Objective-C `Method` (`struct objc_method`). The
/// `Method` type is opaque to apps, but swizzling reads and writes the `imp`
/// field through the `method_*` functions below, so we back each `Method` with
/// a real guest allocation of this shape.
#[repr(C, packed)]
struct objc_method {
    name: SEL,
    types: ConstPtr<u8>,
    imp: GuestFunction,
}
unsafe impl SafeRead for objc_method {}

/// Turn a resolved [IMP] into a guest-callable function address.
///
/// A guest method already has a guest address. A host method does not, so we
/// synthesise an SVC trampoline: when the guest later calls that address as a
/// normal `(id, SEL, ...)` function — which is exactly what swizzled code does
/// with the "original" implementation — the trampoline dispatches straight to
/// the host implementation, because the SVC calling convention matches a method
/// call. This does not run the extra +alloc/dealloc bookkeeping that a full
/// `objc_msgSend` dispatch of a host IMP would; swizzling those methods is out
/// of scope.
fn imp_to_guest_function(env: &mut Environment, imp: IMP) -> GuestFunction {
    match imp {
        IMP::Guest(guest_imp) => guest_imp,
        IMP::Host(host_imp) => {
            let host_function: HostFunction = host_imp;
            env.dyld.create_guest_function(
                &mut env.mem,
                "__tapHLE_swizzled_host_imp",
                host_function,
            )
        }
    }
}

/// `class_getInstanceMethod` — returns the opaque `Method` for an instance
/// method, searching the superclass chain, or `nil` if the class does not
/// respond to the selector.
pub(super) fn class_getInstanceMethod(env: &mut Environment, cls: Class, sel: SEL) -> MutVoidPtr {
    let Some(defining) = env.objc.class_defining_method(cls, sel) else {
        return Ptr::null();
    };

    if let Some(&existing) = env.objc.method_objects.get(&(defining, sel)) {
        return existing;
    }

    let host = env.objc.class_host_object(defining).unwrap();
    let imp = *host.methods.get(&sel).unwrap();
    let types = host
        .guest_method_signatures
        .get(&sel)
        .copied()
        .unwrap_or(Ptr::null());

    let imp = imp_to_guest_function(env, imp);

    let method_ptr: MutPtr<objc_method> = env.mem.alloc(guest_size_of::<objc_method>()).cast();
    env.mem.write(
        method_ptr,
        objc_method {
            name: sel,
            types,
            imp,
        },
    );
    let method_ptr: MutVoidPtr = method_ptr.cast();

    env.objc.method_objects.insert((defining, sel), method_ptr);
    env.objc.method_lookup.insert(method_ptr, (defining, sel));
    method_ptr
}

/// `class_copyMethodList` — every instance method the class itself defines,
/// as a freshly allocated array of `Method` the caller must `free`.
///
/// Only the class's own methods: inherited ones are not included, which is what
/// makes this the function for walking a class hierarchy one level at a time.
/// A class with no methods of its own gives a count of zero and a null pointer,
/// which the documentation specifies and which callers loop over safely.
///
/// The order is by selector name. The real runtime's order is unspecified and
/// tapHLE's own storage is a hash map whose iteration order changes between
/// runs, so sorting is what keeps a swizzling app that walks this list doing
/// the same thing twice in a row.
pub(super) fn class_copyMethodList(
    env: &mut Environment,
    cls: Class,
    out_count: MutPtr<u32>,
) -> MutPtr<MutVoidPtr> {
    let mut selectors: Vec<SEL> = match env.objc.class_host_object(cls) {
        Some(host) => host.methods.keys().copied().collect(),
        None => Vec::new(),
    };
    selectors.sort_by_key(|sel| sel.as_str(&env.mem).to_string());

    if !out_count.is_null() {
        env.mem.write(out_count, selectors.len() as u32);
    }
    if selectors.is_empty() {
        return Ptr::null();
    }

    let list: MutPtr<MutVoidPtr> = env
        .mem
        .alloc(guest_size_of::<MutVoidPtr>() * selectors.len() as GuestUSize)
        .cast();
    for (index, sel) in selectors.into_iter().enumerate() {
        // Goes through the same cache as class_getInstanceMethod, so a Method
        // obtained here and one obtained there are the same pointer - which is
        // what a caller comparing them expects, and what makes swizzling
        // through either of them consistent.
        let method = class_getInstanceMethod(env, cls, sel);
        env.mem.write(list + index as GuestUSize, method);
    }
    list
}

/// `class_getIvarLayout` — the layout string saying which of a class's ivars
/// hold strong references.
///
/// Null is the real answer here, not a placeholder. The layout exists for
/// garbage collection and for ARC's scanning; a class compiled without either -
/// which is every class in a binary of this era - has no layout emitted, and
/// Apple's runtime returns null for it. Callers, typically object-copying and
/// key-value-observing libraries, take null as "nothing to scan".
pub(super) fn class_getIvarLayout(_env: &mut Environment, _cls: Class) -> ConstPtr<u8> {
    Ptr::null()
}

/// `class_getClassMethod` — the class-method counterpart of
/// `class_getInstanceMethod`.
///
/// A class method is an instance method of the metaclass, and a class object's
/// `isa` is its metaclass, so this is the same search one level up. Apps use it
/// the same way as the instance variant: to ask whether a class responds to
/// something before calling it.
pub(super) fn class_getClassMethod(env: &mut Environment, cls: Class, sel: SEL) -> MutVoidPtr {
    if cls == nil {
        return Ptr::null();
    }
    let metaclass = super::ObjC::read_isa(cls, &env.mem);
    class_getInstanceMethod(env, metaclass, sel)
}

/// `class_getMethodImplementation` — returns the `IMP` that a message send of
/// `sel` to an instance of `cls` would invoke, as a guest-callable function
/// pointer, or null if the class does not respond. (The real runtime returns a
/// forwarding handler rather than null in that case; nothing observed needs
/// that yet.)
pub(super) fn class_getMethodImplementation(
    env: &mut Environment,
    cls: Class,
    sel: SEL,
) -> MutVoidPtr {
    let Some(imp) = env.objc.class_get_method_implementation(cls, sel) else {
        return Ptr::null();
    };
    let imp = imp_to_guest_function(env, imp);
    Ptr::from_bits(imp.addr_with_thumb_bit())
}

/// `method_getImplementation` — returns the `IMP` of a `Method` as a
/// guest-callable function pointer.
pub(super) fn method_getImplementation(env: &mut Environment, method: MutVoidPtr) -> MutVoidPtr {
    if method.is_null() {
        return Ptr::null();
    }
    let method: objc_method = env.mem.read(method.cast());
    Ptr::from_bits(method.imp.addr_with_thumb_bit())
}

/// `method_setImplementation` — replaces the `IMP` of a `Method`, returning the
/// previous one. This is the core of method swizzling, so it also updates the
/// defining class's dispatch table so later message sends use the new IMP.
pub(super) fn method_setImplementation(
    env: &mut Environment,
    method: MutVoidPtr,
    imp: MutVoidPtr,
) -> MutVoidPtr {
    if method.is_null() {
        return Ptr::null();
    }
    let method_ptr: MutPtr<objc_method> = method.cast();
    let new_imp = GuestFunction::from_addr_with_thumb_bit(imp.to_bits());

    let mut record = env.mem.read(method_ptr);
    let old_imp = record.imp;
    record.imp = new_imp;
    env.mem.write(method_ptr, record);

    if let Some(&(cls, sel)) = env.objc.method_lookup.get(&method) {
        env.objc
            .borrow_mut::<ClassHostObject>(cls)
            .methods
            .insert(sel, IMP::Guest(new_imp));
    }

    Ptr::from_bits(old_imp.addr_with_thumb_bit())
}

/// `method_getName` — returns the selector of a `Method`.
pub(super) fn method_getName(env: &mut Environment, method: MutVoidPtr) -> SEL {
    if method.is_null() {
        return SEL::null();
    }
    env.mem.read(method.cast::<objc_method>()).name
}

/// `method_getTypeEncoding` — returns the Objective-C type encoding string of a
/// `Method`, or null when one was not recorded.
pub(super) fn method_getTypeEncoding(env: &mut Environment, method: MutVoidPtr) -> ConstPtr<u8> {
    if method.is_null() {
        return Ptr::null();
    }
    env.mem.read(method.cast::<objc_method>()).types
}

/// `method_exchangeImplementations` — atomically swaps the implementations of
/// two methods, the other primitive apps use for swizzling.
pub(super) fn method_exchangeImplementations(
    env: &mut Environment,
    method1: MutVoidPtr,
    method2: MutVoidPtr,
) {
    if method1.is_null() || method2.is_null() || method1 == method2 {
        return;
    }
    let imp1 = method_getImplementation(env, method1);
    let imp2 = method_getImplementation(env, method2);
    method_setImplementation(env, method1, imp2);
    method_setImplementation(env, method2, imp1);
}

/// `class_addMethod` — adds an instance method to a class, failing (returning
/// false) if the class already defines that selector itself.
pub(super) fn class_addMethod(
    env: &mut Environment,
    cls: Class,
    sel: SEL,
    imp: MutVoidPtr,
    types: ConstPtr<u8>,
) -> bool {
    if env.objc.class_host_object(cls).is_none() {
        return false;
    }
    if env.objc.class_has_uninherited_method(cls, sel) {
        return false;
    }
    let imp = GuestFunction::from_addr_with_thumb_bit(imp.to_bits());
    let host = env.objc.borrow_mut::<ClassHostObject>(cls);
    host.methods.insert(sel, IMP::Guest(imp));
    host.guest_method_signatures.insert(sel, types);
    true
}

/// `class_replaceMethod` — adds a method if the class does not define it, or
/// replaces the existing one, returning the previous `IMP` (or null when the
/// method was newly added).
pub(super) fn class_replaceMethod(
    env: &mut Environment,
    cls: Class,
    sel: SEL,
    imp: MutVoidPtr,
    types: ConstPtr<u8>,
) -> MutVoidPtr {
    if env.objc.class_host_object(cls).is_none() {
        return Ptr::null();
    }
    if !env.objc.class_has_uninherited_method(cls, sel) {
        class_addMethod(env, cls, sel, imp, types);
        return Ptr::null();
    }

    let old_imp = *env
        .objc
        .class_host_object(cls)
        .unwrap()
        .methods
        .get(&sel)
        .unwrap();
    let new_imp = GuestFunction::from_addr_with_thumb_bit(imp.to_bits());
    {
        let host = env.objc.borrow_mut::<ClassHostObject>(cls);
        host.methods.insert(sel, IMP::Guest(new_imp));
        host.guest_method_signatures.insert(sel, types);
    }

    // Keep a previously handed-out Method object consistent with the table.
    if let Some(&method_ptr) = env.objc.method_objects.get(&(cls, sel)) {
        let method_ptr: MutPtr<objc_method> = method_ptr.cast();
        let mut record = env.mem.read(method_ptr);
        record.imp = new_imp;
        record.types = types;
        env.mem.write(method_ptr, record);
    }

    let old_imp = imp_to_guest_function(env, old_imp);
    Ptr::from_bits(old_imp.addr_with_thumb_bit())
}
