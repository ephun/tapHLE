/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Handling of Objective-C objects.
//!
//! Note that classes and metaclasses are objects too!
//!
//! Resources:
//! - [Apple's documentation of `id`](https://developer.apple.com/documentation/objectivec/id)
//!   (which for some reason omits that `id` is a pointer type)
//!
//! To make things easier for the host code, our implementation will maintain
//! two linked representations of an object: an [objc_object] struct allocated
//! in guest memory, which needs to maintain the same ABI that Apple's runtime
//! does, and a [HostObject] trait object allocated in host memory, which can be
//! used for any data that only our host code needs to access. As a bonus we get
//! some resilience against guest memory corruption.
//!
//! See also: [crate::frameworks::foundation::ns_object].

use super::{Class, ClassHostObject};
use crate::mem::{guest_size_of, ConstPtr, GuestUSize, Mem, MutPtr, Ptr, SafeRead};
use crate::Environment;
use std::any::Any;
use std::num::NonZeroU32;

/// Memory layout of a minimal Objective-C object. See [id].
///
/// The name comes from `objc_object` in Apple's runtime.
#[repr(C, packed)]
pub struct objc_object {
    /// In life, sometimes we must ask ourselves... what is existence?
    /// What is the meaning in love and suffering? What is it that drives us to
    /// know? What is the joy in longing for absolutes in a universe abundant
    /// in beautiful subjectivity?
    ///
    /// The `isa` pointer cannot answer these questions.
    ///
    /// But it does tell you what class an object belongs to.
    pub(super) isa: Class,
}
unsafe impl SafeRead for objc_object {}

/// Generic pointer to an Objective-C object (including classes or metaclasses).
///
/// The name is standard Objective-C.
#[allow(non_camel_case_types)]
pub type id = MutPtr<objc_object>;

/// Null pointer for Objective-C objects.
///
/// The name is standard Objective-C.
#[allow(non_upper_case_globals)]
pub const nil: id = Ptr::null();

/// Struct used to track the host object and refcount of every object.
/// Maybe debugging info too eventually?
///
/// If the `refcount` is `None`, that means this object has a static duration
/// and should not be reference-counted, e.g. it is a class.
pub(super) struct HostObjectEntry {
    host_object: Box<dyn AnyHostObject>,
    refcount: Option<NonZeroU32>,
    pub(super) cxx_lifecycle: CxxLifecycle,
}

/// State for compiler-generated Objective-C++ ivar construction and teardown.
/// Static objects such as classes do not participate in this lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CxxLifecycle {
    NotApplicable,
    Allocated,
    Constructing,
    Constructed,
    Destructing,
    Destructed,
}

/// Type for host objects.
pub trait HostObject: Any + 'static {
    /// Upcast to the superclass's host object type, if any.
    ///
    /// In order to support inheritance, a class's host object may extend its
    /// superclass's host object. For the superclass to be able to access its
    /// data without being aware of the subclass, it needs to be able to upcast
    /// a host object.
    ///
    /// When trying to downcast from [AnyHostObject] to some type fails, you
    /// should upcast and try again, repeatedly if necessary. This ensures that
    /// if the object actually belongs to a subclass of the target type, the
    /// downcast will eventually succeed. If [None] is returned by this method,
    /// there are no more superclasses' host objects in the chain.
    fn as_superclass<'a>(&'a self) -> Option<&'a (dyn AnyHostObject + 'static)> {
        None
    }
    /// Same as [HostObject::as_superclass], but for a mutable reference.
    fn as_superclass_mut<'a>(&'a mut self) -> Option<&'a mut (dyn AnyHostObject + 'static)> {
        None
    }
}

/// Convenience macro for implementing [HostObject] where the host object type
/// extends some superclass's host object type. The superclass's host object
/// must be in a struct member named `superclass`.
///
/// Example usage, if `Bar` extends `Foo`:
///
/// ```ignore
/// struct BarHostObject {
///     superclass: FooHostObject,
///     some_extra_data: id,
/// }
/// impl_HostObject_with_superclass!(BarHostObject);
/// ```
///
/// If the superclass doesn't have a meaningful host object (e.g. `NSObject`
/// uses [TrivialHostObject]), `impl HostObject for FooHostObject {}` suffices.
#[macro_export]
macro_rules! impl_HostObject_with_superclass {
    ( $ty:ty ) => {
        impl $crate::objc::HostObject for $ty {
            fn as_superclass<'a>(
                &'a self,
            ) -> Option<&'a (dyn $crate::objc::AnyHostObject + 'static)> {
                Some(&self.superclass)
            }
            fn as_superclass_mut<'a>(
                &'a mut self,
            ) -> Option<&'a mut (dyn $crate::objc::AnyHostObject + 'static)> {
                Some(&mut self.superclass)
            }
        }
    };
}
pub use crate::impl_HostObject_with_superclass; // #[macro_export] is weird...

/// Trait wrapping [HostObject] with a blanket implementation to make
/// downcasting work. Don't implement it yourself.
///
/// This is a workaround for it not being possible to directly cast
/// `&'a dyn HostObject` to `&'a dyn Any`.
pub trait AnyHostObject: HostObject {
    fn as_any<'a>(&'a self) -> &'a (dyn Any + 'static);
    fn as_any_mut<'a>(&'a mut self) -> &'a mut (dyn Any + 'static);
    fn type_name(&self) -> &'static str;
}
impl<T: HostObject> AnyHostObject for T {
    fn as_any<'a>(&'a self) -> &'a (dyn Any + 'static) {
        self
    }
    fn as_any_mut<'a>(&'a mut self) -> &'a mut (dyn Any + 'static) {
        self
    }
    fn type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }
}

/// Empty host object used by `[NSObject alloc]`.
pub struct TrivialHostObject;
impl HostObject for TrivialHostObject {}

/// Panic message for an `id` that is not in the object table.
///
/// A bare `unwrap()` here reported only "called `Option::unwrap()` on a `None`
/// value", which says neither which object nor what was wanted from it. The
/// two usual causes are an over-release (the object was deallocated and its
/// entry removed while something still held the pointer) and a pointer that
/// was never a tapHLE object at all, such as a guest-allocated struct being
/// messaged. Naming the object and the expected host type is what makes those
/// distinguishable.
#[cold]
#[inline(never)]
fn missing_host_object<T>(object: id) -> ! {
    panic!(
        "No host object for {object:?}: it is not in the object table, so it was          never allocated by tapHLE or has already been deallocated. Wanted host          type {:?}.",
        std::any::type_name::<T>(),
    )
}

impl super::ObjC {
    /// Read the all-important `isa`.
    pub fn read_isa(object: id, mem: &Mem) -> Class {
        mem.read(object).isa
    }

    /// Overwrite the `isa`, which is what isa-swizzling is.
    ///
    /// Deliberately not paired with any layout check: see [object_setClass],
    /// whose caller is entitled to do this and is responsible for the
    /// consequences.
    pub fn write_isa(object: id, class: Class, mem: &mut Mem) {
        mem.write(object, objc_object { isa: class });
    }

    fn alloc_object_inner(
        &mut self,
        isa: Class,
        instance_size: GuestUSize,
        host_object: Box<dyn AnyHostObject>,
        mem: &mut Mem,
        refcount: Option<NonZeroU32>,
    ) -> id {
        let guest_object = objc_object { isa };
        assert!(instance_size >= guest_size_of::<objc_object>());

        let ptr: MutPtr<objc_object> = mem.alloc(instance_size).cast();
        mem.write(ptr, guest_object);
        assert!(!self.objects.contains_key(&ptr));
        self.objects.insert(
            ptr,
            HostObjectEntry {
                host_object,
                refcount,
                cxx_lifecycle: if refcount.is_some() {
                    CxxLifecycle::Allocated
                } else {
                    CxxLifecycle::NotApplicable
                },
            },
        );
        ptr
    }

    /// Allocate a reference-counted (guest) object (like `[NSObject alloc]`)
    /// and associate it with its host object.
    ///
    /// `isa` must be a real class, as the instance size will be fetched from
    /// the class.
    pub fn alloc_object(
        &mut self,
        isa: Class,
        host_object: Box<dyn AnyHostObject>,
        mem: &mut Mem,
    ) -> id {
        let &ClassHostObject { instance_size, .. } = self.borrow(isa);
        self.alloc_object_inner(
            isa,
            instance_size,
            host_object,
            mem,
            Some(NonZeroU32::new(1).unwrap()),
        )
    }

    /// Allocate a static-lifetime (guest) object (for example, a class) and
    /// associate it with its host object.
    ///
    /// It is assumed that the guest object's instance size is 4 (just an `isa`)
    /// like `NSObject`. This means you must not use this function to implement
    /// the `alloc` method of a class that could be the superclass of any class
    /// in the guest app!
    pub fn alloc_static_object(
        &mut self,
        isa: Class,
        host_object: Box<dyn AnyHostObject>,
        mem: &mut Mem,
    ) -> id {
        let size = guest_size_of::<objc_object>();
        self.alloc_object_inner(isa, size, host_object, mem, None)
    }

    /// Associate a host object with an existing static-lifetime (guest) object
    /// (for example, a class).
    pub fn register_static_object(
        &mut self,
        guest_object: id,
        host_object: Box<dyn AnyHostObject>,
    ) {
        assert!(!self.objects.contains_key(&guest_object));
        self.objects.insert(
            guest_object,
            HostObjectEntry {
                host_object,
                refcount: None,
                cxx_lifecycle: CxxLifecycle::NotApplicable,
            },
        );
    }

    /// Get a reference to a host object, if the object exists.
    pub fn get_host_object(&self, object: id) -> Option<&dyn AnyHostObject> {
        self.objects.get(&object).map(|entry| &*entry.host_object)
    }

    /// Give `object` host storage of type `T` if tapHLE has none for it,
    /// returning whether it had to be adopted.
    ///
    /// Guest code may create an Objective-C instance without going through
    /// `+alloc`. JSONKit does exactly this for its private `JKArray` and
    /// `JKDictionary` collections: it `calloc()`s `class_getInstanceSize()`
    /// bytes, assigns `isa` directly, and then sends `init`. That is legal on
    /// Apple's runtime, where an object is nothing but the memory holding its
    /// `isa` and ivars. tapHLE keeps the other half of every object in a side
    /// table, so an instance made this way is absent from it, and the first
    /// host method to reach for host storage — `retain` and `release` included
    /// — would otherwise abort the emulator.
    ///
    /// Adopting the instance at `init` is the natural repair: `init` is the
    /// message such guest code sends immediately after assigning `isa`, and it
    /// is exactly where tapHLE's own concrete classes set up their storage
    /// anyway. The adopted object gets a refcount of 1, matching what `+alloc`
    /// would have produced.
    pub fn ensure_host_object<T: AnyHostObject + Default>(&mut self, object: id) -> bool {
        if self.objects.contains_key(&object) {
            return false;
        }
        self.objects.insert(
            object,
            HostObjectEntry {
                host_object: Box::<T>::default(),
                refcount: Some(NonZeroU32::new(1).unwrap()),
                cxx_lifecycle: CxxLifecycle::Allocated,
            },
        );
        true
    }

    /// Get a reference to a host object and downcast it. Panics if there is
    /// no such object, or if downcasting fails.
    pub fn borrow<T: AnyHostObject + 'static>(&self, object: id) -> &T {
        let mut host_object: &(dyn AnyHostObject + 'static) = match self.objects.get(&object) {
            Some(entry) => &*entry.host_object,
            None => missing_host_object::<T>(object),
        };
        loop {
            if let Some(res) = host_object.as_any().downcast_ref() {
                return res;
            } else if let Some(next) = host_object.as_superclass() {
                host_object = next;
            } else {
                panic!(
                    "Could not find host object with type {:?}, found {:?} for {object:?}",
                    std::any::type_name::<T>(),
                    host_object.type_name(),
                );
            }
        }
    }

    /// Get a reference to a host object and downcast it. Panics if there is
    /// no such object, or if downcasting fails.
    pub fn borrow_mut<T: AnyHostObject + 'static>(&mut self, object: id) -> &mut T {
        // Rust's borrow checker struggles with loops like this which descend
        // through a data structure with a mutable borrow. The unsafe code is
        // used to bypass the borrow checker.
        type Aho = dyn AnyHostObject + 'static;
        let mut host_object: &mut Aho = match self.objects.get_mut(&object) {
            Some(entry) => &mut *entry.host_object,
            None => missing_host_object::<T>(object),
        };
        loop {
            if let Some(res) = unsafe { &mut *(host_object as *mut Aho) }
                .as_any_mut()
                .downcast_mut()
            {
                return res;
            } else if let Some(next) = host_object.as_superclass_mut() {
                host_object = next;
            } else {
                let host_object: &Aho = &*self.objects.get(&object).unwrap().host_object;
                panic!(
                    "Could not find host object with type {:?}, found {:?} for {object:?}",
                    std::any::type_name::<T>(),
                    host_object.type_name(),
                );
            }
        }
    }

    /// Getting a refcount of an object.
    /// While Apple's docs advise to not relay on the returned value,
    /// some games do call `retainCount`.
    pub fn get_refcount(&mut self, object: id) -> NonZeroU32 {
        let Some(entry) = self.objects.get_mut(&object) else {
            panic!("No entry found for object {object:?}, it may have already been deallocated");
        };
        let Some(refcount) = entry.refcount.as_mut() else {
            // Immortal objects have no count to report. Apple's runtime answers
            // with a saturated value for these rather than treating the
            // question as an error, and so does this.
            return NonZeroU32::new(u32::MAX).unwrap();
        };
        *refcount
    }

    /// Increase the refcount of a reference-counted object. Do not call this
    /// directly unless you're implementing `release` on `NSObject`. That method
    /// may be overridden.
    pub fn increment_refcount(&mut self, object: id) {
        let Some(entry) = self.objects.get_mut(&object) else {
            panic!("No entry found for object {object:?}, it may have already been deallocated");
        };
        let Some(refcount) = entry.refcount.as_mut() else {
            // A static-lifetime object is immortal, and retaining an immortal
            // object is legal and does nothing — on Apple's runtime a constant
            // string or a shared singleton behaves exactly this way. Aborting
            // here treated ordinary, correct guest code as an error: it was the
            // single largest tapHLE-side crash in a survey of 1300 apps.
            //
            // It can still indicate a missing `retain` override on a tapHLE
            // class, which is why it is logged rather than passed over in
            // silence.
            log_once!(
                "Note: retain on a static-lifetime object, which is a no-op. If an object \
                 unexpectedly outlives its owner, a missing retain/release override is one cause."
            );
            return;
        };
        *refcount = refcount.checked_add(1).unwrap();
    }

    /// Decrease the refcount of a reference-counted object. Do not call this
    /// directly unless you're implementing `release` on `NSObject`. That method
    /// may be overridden.
    ///
    /// If the return value is `true`, the object needs to be deallocated. Send
    /// it the `dealloc` message.
    #[must_use]
    pub fn decrement_refcount(&mut self, object: id) -> bool {
        let Some(entry) = self.objects.get_mut(&object) else {
            panic!("No entry found for object {object:?}, it may have already been deallocated");
        };
        let Some(refcount) = entry.refcount.as_mut() else {
            // The counterpart of the retain case above: releasing an immortal
            // object is legal and does nothing. Returning false is what makes
            // it a no-op — it says the object must not be deallocated, which is
            // precisely what "immortal" means.
            return false;
        };
        if refcount.get() == 1 {
            entry.refcount = None;
            true
        } else {
            *refcount = NonZeroU32::new(refcount.get() - 1).unwrap();
            false
        }
    }

    /// Deallocate an object. Do not call this directly unless you're
    /// implementing `dealloc` and are sure you don't need to do a super-call.
    pub fn dealloc_object(&mut self, object: id, mem: &mut Mem) {
        let HostObjectEntry {
            host_object,
            refcount,
            cxx_lifecycle: _,
        } = self.objects.remove(&object).unwrap();

        if let Some(refcount) = refcount {
            // This is a serious bug if it ever happens in host code.
            // Well-behaved apps should also never do this, but Crash Bandicoot
            // Nitro Kart 3D is not a well-behaved app.
            log!(
                "Warning: {:?} was deallocated with non-zero reference count: {:?}",
                object,
                refcount
            );
        }

        std::mem::drop(host_object);

        mem.free(object.cast());
    }

    /// Free an allocation whose Objective-C++ construction failed. Completed
    /// superclass construction has already been unwound by the caller.
    pub(super) fn discard_failed_cxx_construction(&mut self, object: id, mem: &mut Mem) {
        let HostObjectEntry {
            host_object,
            refcount: _,
            cxx_lifecycle,
        } = self.objects.remove(&object).unwrap();
        assert_eq!(cxx_lifecycle, CxxLifecycle::Destructed);
        std::mem::drop(host_object);
        mem.free(object.cast());
    }
}

pub(super) fn object_getClass(env: &mut Environment, obj: id) -> Class {
    if obj == nil {
        nil
    } else {
        super::ObjC::read_isa(obj, &env.mem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objc::ObjC;

    #[derive(Debug, Default, PartialEq)]
    struct TestHostObject {
        value: u32,
    }
    impl HostObject for TestHostObject {}

    /// An instance the guest allocated itself (JSONKit's `calloc()` + `isa`
    /// pattern) is absent from the object table; adopting it must give it
    /// storage and the same refcount `+alloc` would have.
    #[test]
    fn ensure_host_object_adopts_a_guest_allocated_instance() {
        let mut objc = ObjC::new();
        let object: id = Ptr::from_bits(0x2000);

        assert!(objc.get_host_object(object).is_none());
        assert!(objc.ensure_host_object::<TestHostObject>(object));

        assert_eq!(
            objc.borrow::<TestHostObject>(object),
            &TestHostObject::default()
        );
        assert_eq!(objc.get_refcount(object).get(), 1);
    }

    /// Adoption must not disturb an object tapHLE allocated, or a second
    /// `init` would silently discard the storage set up by `+allocWithZone:`.
    #[test]
    fn ensure_host_object_leaves_existing_storage_alone() {
        let mut objc = ObjC::new();
        let mut mem = Mem::new();
        mem.set_null_segment_size(0x1000);
        let object = objc.alloc_static_object(nil, Box::new(TestHostObject { value: 7 }), &mut mem);

        assert!(!objc.ensure_host_object::<TestHostObject>(object));
        assert_eq!(objc.borrow::<TestHostObject>(object).value, 7);
    }

    /// An adopted object must be releasable, since the guest owns it and will
    /// send it `release`/`dealloc` like any other.
    #[test]
    fn an_adopted_object_can_be_released() {
        let mut objc = ObjC::new();
        let object: id = Ptr::from_bits(0x3000);
        assert!(objc.ensure_host_object::<TestHostObject>(object));

        objc.increment_refcount(object);
        assert!(!objc.decrement_refcount(object));
        assert!(objc.decrement_refcount(object));
    }
}

/// The name of an object's class, as a C string.
///
/// Shares `class_getName`'s cache, so repeated calls do not allocate.
/// `object_setClass` — repoint an object's `isa`, returning its previous class.
///
/// This is how isa-swizzling works, and it is deliberately blunt: the runtime
/// does not check that the new class has a compatible instance layout, and
/// neither does this. A caller that swaps in a class with different ivars gets
/// the mess it asked for, on a device as here.
pub(super) fn object_setClass(env: &mut Environment, obj: id, cls: Class) -> Class {
    if obj == nil {
        return nil;
    }
    let previous = super::ObjC::read_isa(obj, &env.mem);
    super::ObjC::write_isa(obj, cls, &mut env.mem);
    previous
}

pub(super) fn object_getClassName(env: &mut Environment, obj: id) -> ConstPtr<u8> {
    let class = object_getClass(env, obj);
    super::classes::class_getName(env, class)
}
