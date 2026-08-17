/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Handling of Objective-C classes and metaclasses.
//!
//! Note that metaclasses are just a special case of classes.
//!
//! Resources:
//! - [[objc explain]: Classes and metaclasses](http://www.sealiesoftware.com/blog/archive/2009/04/14/objc_explain_Classes_and_metaclasses.html), especially [the PDF diagram](http://www.sealiesoftware.com/blog/class%20diagram.pdf)

use super::{
    id, ivar_list_t, method_list_t, nil, objc_object, objc_property_t, property_list_t,
    AnyHostObject, HostIMP, HostObject, IvarInfo, ObjC, IMP, SEL,
};
use crate::mach_o::MachO;
use crate::mem::{guest_size_of, ConstPtr, ConstVoidPtr, GuestUSize, Mem, MutPtr, Ptr, SafeRead};
use crate::Environment;
use std::collections::{HashMap, VecDeque};

/// Generic pointer to an Objective-C class or metaclass.
///
/// The name is standard Objective-C.
///
/// Apple's runtime has a `objc_classes` definition similar to `objc_object`.
/// We could do the same thing here, but it doesn't seem worth it, as we can't
/// get the same unidirectional type safety.
pub type Class = id;

/// Our internal representation of a class, e.g. this is where `objc_msgSend`
/// will look up method implementations.
///
/// Note: `superclass` can be `nil`!
pub(super) struct ClassHostObject {
    pub(super) name: String,
    pub(super) is_metaclass: bool,
    pub(super) superclass: Class,
    pub(super) methods: HashMap<SEL, IMP>,
    pub(super) guest_method_signatures: HashMap<SEL, ConstPtr<u8>>,
    /// Maps ivar name to what the class knows about it: the binary's descriptor
    /// for it, its offset global, and its alignment.
    pub(super) ivars: HashMap<String, IvarInfo>,
    /// Declared properties, in declaration order, each paired with a pointer to
    /// its entry in the binary's property table. Only the class's own
    /// properties; the superclass chain is walked at lookup time.
    pub(super) properties: Vec<(String, objc_property_t)>,
    /// The class's own adopted-protocol list in the binary, or null. Read on
    /// demand rather than expanded at load time: only `-conformsToProtocol:`
    /// asks, and most classes are never asked.
    pub(super) base_protocols: ConstPtr<protocol_list_t>,
    /// Offset into the allocated memory for the object where the ivars of
    /// instances of this class or metaclass (respectively: normal objects or
    /// classes) should live. This is always >= the value in the superclass.
    pub(super) instance_start: GuestUSize,
    /// Size of the allocated memory for instances of this class or metaclass.
    /// This is always >= the value in the superclass.
    pub(super) instance_size: GuestUSize,
    /// Checks if +initialize has been called yet.
    pub(super) is_initialized: InitializationStatus,
}
impl HostObject for ClassHostObject {}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum InitializationStatus {
    NotInitialized,
    Initializing,
    Initialized,
}

fn reconcile_ivar_offsets(
    mem: &mut Mem,
    ivars: &HashMap<String, IvarInfo>,
    mut diff: GuestUSize,
) -> GuestUSize {
    let mut max_alignment: GuestUSize = 1;
    for &IvarInfo {
        offset,
        alignment: alignment_raw,
        ..
    } in ivars.values()
    {
        if offset.is_null() {
            // Anonymous bitfield.
            continue;
        }
        // Objective-C metadata stores log2(alignment), except that UINT32_MAX
        // means the guest word size. This matches ivar_t::alignment() in
        // Apple's runtime.
        let alignment = if alignment_raw == u32::MAX {
            guest_size_of::<GuestUSize>()
        } else {
            1_u32
                .checked_shl(alignment_raw)
                .expect("Invalid Objective-C ivar alignment")
        };
        max_alignment = max_alignment.max(alignment);
    }

    let align_mask = max_alignment - 1;
    diff = (diff + align_mask) & !align_mask;

    for &IvarInfo { offset, .. } in ivars.values() {
        if offset.is_null() {
            continue;
        }

        // The compiler-generated getter loads the ivar offset through this
        // pointer, so reconciliation must update the pointed-to value. Moving
        // our bookkeeping pointer instead leaves guest code using the stale
        // offset.
        let old_offset = mem.read(offset);
        let new_offset = old_offset
            .checked_add(diff)
            .expect("Objective-C ivar offset overflow");
        mem.write(offset.cast_mut(), new_offset);
    }

    diff
}

/// Placeholder object for classes and metaclasses referenced by the app that
/// we don't have an implementation for.
///
/// This lets us delay errors about missing implementations until the first
/// time the app actually uses them (e.g. when a message is sent).
pub(super) struct UnimplementedClass {
    pub(super) name: String,
    pub(super) is_metaclass: bool,
}
impl HostObject for UnimplementedClass {}

/// Substitute object for classes and metaclasses from the guest app that we do
/// not want to support (see [substitute_classes]).
///
/// Messages sent to this class will behave as if messaging [nil].
pub(super) struct FakeClass {
    pub(super) name: String,
    pub(super) is_metaclass: bool,
}
impl HostObject for FakeClass {}

/// The layout of a class in an app binary.
///
/// The name, field names and field layout are based on what Ghidra outputs.
#[repr(C, packed)]
#[allow(dead_code)]
struct class_t {
    isa: Class, // note that this matches objc_object
    superclass: Class,
    _cache: ConstVoidPtr,
    _vtable: ConstVoidPtr,
    data: ConstPtr<class_rw_t>,
}
unsafe impl SafeRead for class_t {}

/// The layout of the main class data in an app binary.
///
/// The name, field names and field layout are based on what Ghidra's output.
#[repr(C, packed)]
#[allow(dead_code)]
struct class_rw_t {
    _flags: u32,
    instance_start: GuestUSize,
    instance_size: GuestUSize,
    _reserved: u32,
    name: ConstPtr<u8>,
    base_methods: ConstPtr<method_list_t>,
    base_protocols: ConstPtr<protocol_list_t>,
    ivars: ConstPtr<ivar_list_t>,
    _weak_ivar_layout: u32,
    base_properties: ConstPtr<property_list_t>,
}
unsafe impl SafeRead for class_rw_t {}

/// The layout of a protocol list in an app binary: a count followed by that
/// many pointers to protocols. Both the class's own list and a protocol's list
/// of protocols it adopts have this shape.
#[repr(C, packed)]
pub(super) struct protocol_list_t {
    count: GuestUSize,
    // Followed by `count` pointers to protocol_t, read individually.
}
unsafe impl SafeRead for protocol_list_t {}

/// As much of a protocol as answering "does this class adopt it?" needs: its
/// name, and the protocols it adopts itself. A protocol carries its method
/// lists after these, which nothing here reads.
#[repr(C, packed)]
#[allow(dead_code)]
struct protocol_t {
    isa: ConstVoidPtr,
    name: ConstPtr<u8>,
    protocols: ConstPtr<protocol_list_t>,
}
unsafe impl SafeRead for protocol_t {}

/// The layout of a category in an app binary.
///
/// The name, field names and field layout are based on what Ghidra outputs.
#[repr(C, packed)]
struct category_t {
    name: ConstPtr<u8>,
    class: Class,
    instance_methods: ConstPtr<method_list_t>,
    class_methods: ConstPtr<method_list_t>,
    _protocols: ConstVoidPtr,     // protocol list (TODO)
    _property_list: ConstVoidPtr, // property list (TODO)
}
unsafe impl SafeRead for category_t {}

/// A template for a class defined with [objc_classes].
///
/// Host implementations of libraries can use these to expose classes to the
/// application. The runtime will create the actual class ([ClassHostObject]
/// etc) from the template on-demand. See also [ClassExports].
pub struct ClassTemplate {
    pub name: &'static str,
    pub superclass: Option<&'static str>,
    pub class_methods: &'static [(&'static str, &'static dyn HostIMP)],
    pub instance_methods: &'static [(&'static str, &'static dyn HostIMP)],
}

/// Type for lists of classes exported by host implementations of frameworks.
///
/// Each module that wants to expose functions to guest code should export a
/// constant using this type. See [objc_classes] for an example.
///
/// All the constants like this can then be collected into a
/// [crate::dyld::HostDylib].
///
/// The strings are the class names.
///
/// See also [crate::dyld::ConstantExports] and [crate::dyld::FunctionExports].
pub type ClassExports = &'static [(&'static str, ClassTemplate)];

#[doc(hidden)]
#[macro_export]
macro_rules! _objc_superclass {
    (: $name:ident) => {
        Some(stringify!($name))
    };
    () => {
        None
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! _objc_method {
    (
        $env:ident,
        $this:ident,
        $_cmd:ident,
        $cmd_name:ident,
        $retty:ty,
        $block:block
        $(, $ty:ty, $arg:ident)*
        $(, ...$va_arg:ident: $va_type:ty)?
    ) => {
        // The closure must be explicitly casted because a bare closure defaults
        // to a different type than a pure fn pointer, which is the type that
        // HostIMP and CallFromGuest are implemented on.
        &((|
            #[allow(unused_variables)]
            $env: &mut $crate::Environment,
            #[allow(unused_variables)]
            $this: $crate::objc::id,
            #[allow(unused_variables)]
            $_cmd: $crate::objc::SEL,
            $($arg: $ty,)*
            $(#[allow(unused_mut)] mut $va_arg: $va_type,)?
        | -> $retty {
            const _OBJC_CURRENT_SELECTOR: &str = stringify!($cmd_name);
            $block
        }) as fn(
            &mut $crate::Environment,
            $crate::objc::id,
            $crate::objc::SEL,
            $($ty,)*
            $($va_type,)?
        ) -> $retty)
    }
}

/// Macro for creating a list of [ClassTemplate]s (i.e. [ClassExports]).
/// It imitates the Objective-C class definition syntax.
///
/// ```ignore
/// pub const CLASSES: ClassExports = objc_classes! {
/// (env, this, _cmd); // Specify names of HostIMP implicit parameters.
///                    // The second one should be `self` to match Objective-C,
///                    // but that's reserved in Rust, hence `this`.
///
/// @implementation MyClass: NSObject
///
/// + (id)foo {
///     // ...
/// }
///
/// - (id)barWithQux:(u32)qux {
///     // ...
/// }
///
/// - (id)barWithVaArgs:(u32)qux, ...dots {
///     // ...
/// }
///
/// @end
/// };
/// ```
///
/// will desugar to approximately:
///
/// ```ignore
/// pub const CLASSES: ClassExports = &[
///     ("MyClass", ClassTemplate {
///         name: "MyClass",
///         superclass: Some("NSObject"),
///         class_methods: &[
///             ("foo", &(|env: &mut Environment, this: id, _cmd: SEL| -> id {
///                 // ...
///             } as fn(&mut Environment, id, SEL) -> id)),
///         ],
///         instance_methods: &[
///             ("barWithQux:", &(|
///                 env: &mut Environment,
///                 this: id,
///                 _cmd: SEL,
///                 qux: u32
///             | -> id {
///                 // ...
///             } as &fn(&mut Environment, id, SEL, u32) -> id)),
///             ("barWithVaArgs:", &(|
///                 env: &mut Environment,
///                 this: id,
///                 _cmd: SEL,
///                 qux: u32,
///                 va_args: DotDotDot
///             | -> id {
///                 // ...
///             } as &fn(&mut Environment, id, SEL, u32, DotDotDot) -> id)),
///         ],
///     })
/// ];
/// ```
///
/// Note that the instance methods must be preceded by the class methods.
#[macro_export] // documentation comment links are annoying without this
macro_rules! objc_classes {
    {
        // Rust's macro hygiene prevents the macro's own names for these
        // parameters being visible, so we have to get names supplied by the
        // macro user.
        ($env:ident, $this:ident, $_cmd:ident);
        $(
            @implementation $class_name:ident $(: $superclass_name:ident)?

            $( + ($cm_type:ty) $cm_name:ident $(:($cm_type1:ty) $cm_arg1:ident $($($cm_namen:ident)?:($cm_typen:ty) $cm_argn:ident)*)?
                              $(, ...$cm_va_arg:ident)?
                 $cm_block:block )*

            $( - ($im_type:ty) $im_name:ident $(:($im_type1:ty) $im_arg1:ident $($($im_namen:ident)?:($im_typen:ty) $im_argn:ident)*)?
                              $(, ...$im_va_arg:ident)?
                 $im_block:block )*

            @end
        )+
    } => {
        &[
            $({
                // This constant is for `msg_super!`, which needs to know which
                // class it is has been written within (not the same as the
                // runtime type of `this`, which could be a subclass). This is
                // a constant instead of a let binding because that escapes
                // Rust's macro hygiene.
                const _OBJC_CURRENT_CLASS: &str = stringify!($class_name);

                (_OBJC_CURRENT_CLASS, $crate::objc::ClassTemplate {
                    name: _OBJC_CURRENT_CLASS,
                    superclass: $crate::_objc_superclass!($(: $superclass_name)?),
                    class_methods: &[
                        $(
                            (
                                $crate::objc::selector!(
                                    $(($cm_type1);)?
                                    $cm_name
                                    $($(, $($cm_namen)?)*)?
                                ),
                                $crate::_objc_method!(
                                    $env,
                                    $this,
                                    $_cmd,
                                    $cm_name,
                                    $cm_type,
                                    { $cm_block }
                                    $(, $cm_type1, $cm_arg1 $(, $cm_typen, $cm_argn)*)?
                                    $(, ...$cm_va_arg: $crate::abi::DotDotDot)?
                                )
                            )
                        ),*
                    ],
                    instance_methods: &[
                        $(
                            (
                                $crate::objc::selector!(
                                    $(($im_type1);)?
                                    $im_name
                                    $($(, $($im_namen)?)*)?
                                ),
                                $crate::_objc_method!(
                                    $env,
                                    $this,
                                    $_cmd,
                                    $im_name,
                                    $im_type,
                                    { $im_block }
                                    $(, $im_type1, $im_arg1 $(, $im_typen, $im_argn)*)?
                                    $(, ...$im_va_arg: $crate::abi::DotDotDot)?
                                )
                            )
                        ),*
                    ],
                })
            }),+
        ]
    }
}
pub use crate::objc_classes; // #[macro_export] is weird...

/// Whether a protocol list, or any protocol it leads to, names `wanted`.
///
/// A protocol may itself adopt protocols, and conformance is inherited through
/// them, so this follows those lists too. The depth limit is protection against
/// a malformed or circular list in a binary rather than a real nesting bound;
/// protocol hierarchies are a couple of levels deep in practice.
fn protocol_list_contains(
    mem: &Mem,
    list: ConstPtr<protocol_list_t>,
    wanted: &str,
    depth: u32,
) -> bool {
    if list.is_null() || depth > 8 {
        return false;
    }
    let protocol_list_t { count } = mem.read(list);
    // The pointers follow the count in memory.
    let entries: ConstPtr<ConstPtr<protocol_t>> = (list + 1u32).cast();
    for i in 0..count {
        let protocol = mem.read(entries + i);
        if protocol.is_null() {
            continue;
        }
        let protocol_t {
            name, protocols, ..
        } = mem.read(protocol);
        if !name.is_null() && mem.cstr_at_utf8(name) == Ok(wanted) {
            return true;
        }
        if protocol_list_contains(mem, protocols, wanted, depth + 1) {
            return true;
        }
    }
    false
}

impl ClassHostObject {
    fn from_template(
        template: &ClassTemplate,
        is_metaclass: bool,
        superclass: Class,
        objc: &ObjC,
    ) -> Self {
        // For our host implementations we store all data in host objects, so
        // there are no ivars and the size is always just the isa pointer.
        // This is true for both classes and normal objects.
        let size = guest_size_of::<objc_object>();
        ClassHostObject {
            name: template.name.to_string(),
            is_metaclass,
            superclass,
            base_protocols: Ptr::null(),
            methods: HashMap::from_iter(
                (if is_metaclass {
                    template.class_methods
                } else {
                    template.instance_methods
                })
                .iter()
                .map(|&(name, host_imp)| {
                    // The selector should already have been registered by
                    // [ObjC::register_host_selectors], so we can panic
                    // if it hasn't been.
                    (objc.selectors[name], IMP::Host(host_imp))
                }),
            ),
            guest_method_signatures: HashMap::default(),
            // maybe this should be 0 for NSObject? does it matter?
            instance_start: size,
            instance_size: size,
            ivars: HashMap::default(),
            properties: Vec::new(),
            is_initialized: InitializationStatus::NotInitialized,
        }
    }

    fn from_bin(class: Class, is_metaclass: bool, mem: &Mem, objc: &mut ObjC) -> Self {
        let class_t {
            superclass, data, ..
        } = mem.read(class.cast());
        let class_rw_t {
            instance_start,
            instance_size,
            name,
            base_methods,
            base_protocols,
            ivars,
            base_properties,
            ..
        } = mem.read(data);

        let name = mem.cstr_at_utf8(name).unwrap().to_string();

        let mut host_object = ClassHostObject {
            name,
            is_metaclass,
            superclass,
            base_protocols,
            methods: HashMap::new(),
            guest_method_signatures: HashMap::new(),
            instance_start,
            instance_size,
            ivars: HashMap::new(),
            properties: Vec::new(),
            is_initialized: InitializationStatus::NotInitialized,
        };

        if !base_methods.is_null() {
            host_object.add_methods_from_bin(base_methods, mem, objc);
        }

        if !ivars.is_null() {
            host_object.add_ivars_from_bin(ivars, mem);
        }

        if !base_properties.is_null() {
            host_object.add_properties_from_bin(base_properties, mem);
        }

        host_object
    }

    // See methods.rs for binary method parsing
}

/// Decide whether a certain class/metaclass pair from the guest app should use
/// fake class host objects and return the substitutions if so.
///
/// This function is called when registering classes from the guest app. It
/// detects certain problematic classes that are, for example, too complex for
/// tapHLE to currently support, but which can be easily replaced with simple
/// fakes.
fn substitute_classes(
    mem: &Mem,
    class: Class,
    metaclass: Class,
) -> Option<(Box<FakeClass>, Box<FakeClass>)> {
    let class_t { data, .. } = mem.read(class.cast());
    let class_rw_t { name, .. } = mem.read(data);
    let name = mem.cstr_at_utf8(name).unwrap();

    // Currently the only thing we try to substitute: classes that seem to be
    // from various third-party advertising or social network SDKs.
    // Naturally it makes a lot of use of UIKit and networking in ways we
    // don't support yet. This isn't "ad blocking" because ads no longer work
    // on real devices anyway :)
    if !(name.starts_with("AdMob")
        || name.starts_with("AltAds")
        || name.starts_with("Mobclix")
        || name.starts_with("FB") // Facebook
        || name.starts_with("Flurry")
        || name.starts_with("OpenFeint")
        || name.starts_with("Tapjoy"))
    {
        return None;
    }

    {
        let class_t { data, .. } = mem.read(metaclass.cast());
        let class_rw_t {
            name: metaclass_name,
            ..
        } = mem.read(data);
        let metaclass_name = mem.cstr_at_utf8(metaclass_name).unwrap();
        assert!(name == metaclass_name);
    }

    log!(
        "Note: substituting fake class for {} to improve compatibility",
        name
    );

    let class_host_object = Box::new(FakeClass {
        name: name.to_string(),
        is_metaclass: false,
    });
    let metaclass_host_object = Box::new(FakeClass {
        name: name.to_string(),
        is_metaclass: true,
    });
    Some((class_host_object, metaclass_host_object))
}

impl ObjC {
    /// Iterator over all known classes and their names.
    pub fn all_classes(&self) -> impl Iterator<Item = (&String, &Class)> {
        self.classes.iter()
    }

    fn get_class(&self, name: &str, is_metaclass: bool, mem: &Mem) -> Option<Class> {
        let class = self.classes.get(name).copied()?;
        Some(if is_metaclass {
            Self::read_isa(class, mem)
        } else {
            class
        })
    }

    fn find_template(name: &str) -> Option<&'static ClassTemplate> {
        crate::dyld::search_host_dylibs(|dylib| dylib.class_exports, name)
            .map(|&(_name, ref template)| template)
    }

    /// For use by [crate::dyld]: get the class or metaclass referenced by an
    /// external relocation in the app binary. If we don't have an
    /// implementation of the class, a placeholder is used.
    pub fn link_class(&mut self, name: &str, is_metaclass: bool, mem: &mut Mem) -> Class {
        self.link_class_inner(name, is_metaclass, mem, true)
    }

    /// For use by host functions: get a particular class. If we don't have an
    /// implementation of the class, panic.
    pub fn get_known_class(&mut self, name: &str, mem: &mut Mem) -> Class {
        self.link_class_inner(name, /* is_metaclass: */ false, mem, false)
    }

    /// Like [Self::get_known_class], but answers [None] instead of panicking
    /// when tapHLE has no implementation of the class.
    ///
    /// This is for the feature-detection idiom: an app that asks
    /// `NSClassFromString(@"GKLeaderboardViewController")` is explicitly
    /// asking whether the class exists on this OS version, and "no" is a
    /// legitimate answer it is written to handle. Do not use this where the
    /// caller actually requires the class — there the panic is the useful
    /// signal that something is missing.
    pub fn get_known_class_if_implemented(&mut self, name: &str, mem: &mut Mem) -> Option<Class> {
        if self
            .get_class(name, /* is_metaclass: */ false, mem)
            .is_none()
            && Self::find_template(name).is_none()
        {
            return None;
        }
        Some(self.get_known_class(name, mem))
    }

    fn link_class_inner(
        &mut self,
        name: &str,
        is_metaclass: bool,
        mem: &mut Mem,
        use_placeholder: bool,
    ) -> Class {
        // The class and metaclass must be created together and tracked
        // together, so even though this function only returns one pointer, it
        // must create both. The function must not care whether the metaclass
        // is requested first, or if the class is requested first.

        if let Some(class) = self.get_class(name, is_metaclass, mem) {
            return class;
        };

        let class_host_object: Box<dyn AnyHostObject>;
        let metaclass_host_object: Box<dyn AnyHostObject>;
        if let Some(template) = Self::find_template(name) {
            // We have a template (host implementation) for this class, use it.

            if let Some(superclass_name) = template.superclass {
                // Make sure we actually have a template for the superclass
                // before we try to link it, else we might get an unimplemented
                // class back and have weird problems down the line
                assert!(Self::find_template(superclass_name).is_some());
            }

            class_host_object = Box::new(ClassHostObject::from_template(
                template,
                /* is_metaclass: */ false,
                /* superclass: */
                template
                    .superclass
                    .map(|name| {
                        self.link_class(name, /* is_metaclass: */ false, mem)
                    })
                    .unwrap_or(nil),
                self,
            ));
            metaclass_host_object = Box::new(ClassHostObject::from_template(
                template,
                /* is_metaclass: */ true,
                /* superclass: */
                template
                    .superclass
                    .map(|name| {
                        self.link_class(name, /* is_metaclass: */ true, mem)
                    })
                    .unwrap_or(nil),
                self,
            ));
        } else {
            if !use_placeholder {
                panic!("Missing implementation for class {name}!");
            }

            // We don't have a real implementation for this class, use a
            // placeholder.

            class_host_object = Box::new(UnimplementedClass {
                name: name.to_string(),
                is_metaclass: false,
            });
            metaclass_host_object = Box::new(UnimplementedClass {
                name: name.to_string(),
                is_metaclass: true,
            });
        }

        // NSObject's metaclass is special: it is its own metaclass, and it's
        // the superclass of all other metaclasses.
        // (FIXME: this actually should apply to any class hiearchy root.)
        // This creates a chicken-and-egg problem so it has a special path.
        let metaclass = if name == "NSObject" {
            let metaclass = mem.alloc_and_write(objc_object { isa: nil });
            mem.write(metaclass, objc_object { isa: metaclass });
            self.register_static_object(metaclass, metaclass_host_object);
            metaclass
        } else {
            let isa = self.link_class("NSObject", /* is_metaclass: */ true, mem);
            self.alloc_static_object(isa, metaclass_host_object, mem)
        };

        let class = self.alloc_static_object(metaclass, class_host_object, mem);

        if name == "NSObject" {
            // NSObject's metaclass has its class as the superclass.
            self.borrow_mut::<ClassHostObject>(metaclass).superclass = class;
        }

        self.classes.insert(name.to_string(), class);

        if is_metaclass {
            metaclass
        } else {
            class
        }
    }

    /// Point a guest class that subclasses an abstract Foundation class at
    /// tapHLE's concrete implementation of it instead.
    ///
    /// Foundation's collection and string classes are class clusters: `NSArray`
    /// declares the interface and a private subclass provides the storage. In
    /// tapHLE the storage and the primitives — `count`, `objectAtIndex:` and so
    /// on — live on `_tapHLE_NSArray`, and `NSArray` itself has none of them.
    ///
    /// An app that subclasses `NSArray` to add a category-like helper therefore
    /// inherits an interface with no implementation behind it, and its
    /// instances have nowhere to put elements. Before this, such a class either
    /// tripped an assertion in `+allocWithZone:` or was quietly handed a plain
    /// concrete array, losing its own identity and its own methods — which is
    /// how a JSON-serialisation category dies on `-serialize`.
    ///
    /// Splicing the concrete class into the chain fixes both halves at once:
    /// the subclass keeps its identity and its methods, and inherits real
    /// storage. The metaclass is re-parented alongside the class, which is the
    /// part that matters for `+alloc` — otherwise allocation still resolves to
    /// the abstract class's and hands back the wrong object.
    ///
    /// This is sound only because tapHLE's concrete classes add no guest ivars:
    /// their instances are an `isa` and nothing else, exactly like the abstract
    /// class the compiler laid the subclass out against. If that ever stops
    /// being true, the subclass's ivar offsets would need shifting and this
    /// would silently corrupt them, so it is checked rather than assumed.
    fn reparent_onto_concrete_classes(&mut self, registered: &[Class], mem: &mut Mem) {
        /// Abstract class an app may subclass, and the concrete class that
        /// actually implements it.
        const SUBSTITUTIONS: &[(&str, &str)] = &[
            ("NSArray", "_tapHLE_NSArray"),
            ("NSMutableArray", "_tapHLE_NSMutableArray"),
            ("NSString", "_tapHLE_NSString"),
            ("NSMutableString", "_tapHLE_NSMutableString"),
            ("NSDictionary", "_tapHLE_NSDictionary"),
            ("NSMutableDictionary", "_tapHLE_NSMutableDictionary"),
        ];

        for &(abstract_name, concrete_name) in SUBSTITUTIONS {
            // Only look at apps that actually reference the abstract class;
            // asking for it here would otherwise create every one of these.
            let Some(abstract_class) = self.get_class(abstract_name, false, mem) else {
                continue;
            };

            let mut concrete_class = None;
            for &class in registered {
                let Some(&ClassHostObject { superclass, .. }) = self
                    .get_host_object(class)
                    .and_then(|host_object| host_object.as_any().downcast_ref())
                else {
                    continue;
                };
                if superclass != abstract_class {
                    continue;
                }

                let concrete =
                    *concrete_class.get_or_insert_with(|| self.get_known_class(concrete_name, mem));

                let abstract_size = {
                    let host_object: &ClassHostObject = self.borrow(abstract_class);
                    host_object.instance_size
                };
                let concrete_size = {
                    let host_object: &ClassHostObject = self.borrow(concrete);
                    host_object.instance_size
                };
                if abstract_size != concrete_size {
                    log!(
                        "Warning: not re-parenting a subclass of {} onto {}: their instance sizes differ ({} vs {}), so the subclass's ivars would move",
                        abstract_name,
                        concrete_name,
                        abstract_size,
                        concrete_size
                    );
                    continue;
                }

                let name = {
                    let host_object: &ClassHostObject = self.borrow(class);
                    host_object.name.clone()
                };
                log_dbg!(
                    "Re-parenting guest class {:?} from {} onto {}",
                    name,
                    abstract_name,
                    concrete_name
                );
                self.borrow_mut::<ClassHostObject>(class).superclass = concrete;

                // And the metaclass, so +alloc finds the concrete allocator.
                let metaclass = Self::read_isa(class, mem);
                let concrete_metaclass = Self::read_isa(concrete, mem);
                if let Some(host_object) = self.get_host_object(metaclass) {
                    if host_object
                        .as_any()
                        .downcast_ref::<ClassHostObject>()
                        .is_some()
                    {
                        self.borrow_mut::<ClassHostObject>(metaclass).superclass =
                            concrete_metaclass;
                    }
                }
            }
        }
    }

    /// For use by [crate::dyld]: register all the classes from the application
    /// binary.
    pub fn register_bin_classes(&mut self, bin: &MachO, mem: &mut Mem) {
        let Some(list) = bin.get_section("__objc_classlist") else {
            return;
        };

        assert!(list.size % 4 == 0);
        let base: ConstPtr<Class> = Ptr::from_bits(list.addr);
        let mut registered: Vec<Class> = Vec::new();
        for i in 0..(list.size / 4) {
            let class = mem.read(base + i);
            let metaclass = Self::read_isa(class, mem);

            let name = if let Some(fakes) = substitute_classes(mem, class, metaclass) {
                let (class_host_object, metaclass_host_object) = fakes;

                assert!(class_host_object.name == metaclass_host_object.name);
                let name = class_host_object.name.clone();

                self.register_static_object(class, class_host_object);
                self.register_static_object(metaclass, metaclass_host_object);
                name
            } else {
                let class_host_object = Box::new(ClassHostObject::from_bin(
                    class, /* is_metaclass: */ false, mem, self,
                ));
                let metaclass_host_object = Box::new(ClassHostObject::from_bin(
                    metaclass, /* is_metaclass: */ true, mem, self,
                ));

                assert!(class_host_object.name == metaclass_host_object.name);
                let name = class_host_object.name.clone();

                self.register_static_object(class, class_host_object);
                self.register_static_object(metaclass, metaclass_host_object);
                name
            };

            self.classes.insert(name.to_string(), class);
            registered.push(class);
        }

        self.reparent_onto_concrete_classes(&registered, mem);

        let mut queue = VecDeque::<Class>::new();
        let mut found_ns_object = false;
        // Second pass to build an inverted inheritance graph
        let mut inverted_inheritance = HashMap::<Class, Vec<Class>>::new();
        for (name, class) in self.classes.iter() {
            log_dbg!("class name {}", name);
            if name == "NSObject" {
                assert!(!found_ns_object);
                found_ns_object = true;
            }
            let class_host_object = self
                .get_host_object(*class)
                .unwrap()
                .as_any()
                .downcast_ref();
            let Some(ClassHostObject { superclass, .. }) = class_host_object else {
                // Skip FakeClass or UnimplementedClass
                continue;
            };

            if *superclass != nil {
                inverted_inheritance
                    .entry(*superclass)
                    .and_modify(|v| v.push(*class))
                    .or_insert(vec![*class]);
            } else {
                assert!(!queue.contains(class));
                queue.push_back(*class);
            }
        }
        // At least NSObject should be found as a root object
        assert!(found_ns_object);

        // Third pass to ensure no superclass has "grown into" any of its
        // subclasses.
        // (https://alwaysprocessing.blog/2023/03/12/objc-ivar-abi)

        // BFS starting from root(s) for ivar reconciliation
        //
        // It is required to be traversed in this order,
        // as one overgrown class potentially implies
        // reconciliation for _all of subclasses_
        while !queue.is_empty() {
            let next = queue.pop_front().unwrap();
            let (need, mut diff) = self.need_ivar_reconciliation(next);
            if need {
                let ClassHostObject {
                    name, superclass, ..
                } = self.borrow(next);
                log_dbg!(
                    "Class {} need ivar reconciliation with superclass {}!",
                    name,
                    &self.borrow::<ClassHostObject>(*superclass).name
                );

                let ClassHostObject {
                    ref mut instance_start,
                    ref mut instance_size,
                    ref ivars,
                    ..
                } = self.borrow_mut(next);

                if !ivars.is_empty() {
                    diff = reconcile_ivar_offsets(mem, ivars, diff);
                }

                *instance_start += diff;
                *instance_size += diff;
            }
            if let Some(subclasses) = inverted_inheritance.get(&next) {
                queue.extend(subclasses);
            }
        }
    }

    fn need_ivar_reconciliation(&mut self, class: Class) -> (bool, u32) {
        let class_host_object = self.get_host_object(class).unwrap().as_any().downcast_ref();
        let Some(ClassHostObject {
            name,
            superclass,
            instance_start,
            instance_size,
            ..
        }) = class_host_object
        else {
            // The class might be a FakeClass or UnimplementedClass
            // In those cases we move on as they don't have ivars
            return (false, 0);
        };
        log_dbg!(
            "Checking need_ivar_reconciliation for {}, start {}, size {}",
            name,
            instance_start,
            instance_size
        );

        if *superclass == nil {
            return (false, 0);
        }

        let superclass_host_object = self
            .get_host_object(*superclass)
            .unwrap()
            .as_any()
            .downcast_ref();
        let Some(ClassHostObject {
            instance_size: superclass_instance_size,
            ..
        }) = superclass_host_object
        else {
            // Superclass could also be a FakeClass or UnimplementedClass
            return (false, 0);
        };

        let need = instance_start < superclass_instance_size;
        let diff = if need {
            superclass_instance_size - instance_start
        } else {
            0
        };
        (need, diff)
    }

    /// Dumps all classes available to the emulator in JSON to stdout.
    ///
    /// The JSON has the following form:
    /// ```json
    /// {
    ///     "object": "classes",
    ///     "classes": [
    ///         {
    ///             "name": ((name of class)),
    ///             "super": ((name of superclass, if available)),
    ///             "class_type": (("normal" | "unimplemented" | "fake"))
    ///         },
    ///         ...
    ///     ]
    /// }
    /// ```
    pub fn dump_classes(&self, file: &mut std::fs::File) -> Result<(), std::io::Error> {
        use std::io::Write;
        writeln!(file, "{{\n    \"object\": \"classes\",\n    \"classes\": [")?;
        for (i, (_, o)) in self.classes.iter().enumerate() {
            // Why doesn't json allow trailing commas...
            let comma = if i == self.classes.len() - 1 { "" } else { "," };

            let host_obj = self.get_host_object(*o).unwrap();

            if let Some(ClassHostObject {
                name,
                superclass: sup,
                ..
            }) = host_obj.as_any().downcast_ref()
            {
                if *sup == nil {
                    writeln!(
                        file,
                        "        {{ \"name\": \"{name}\", \"class_type\": \"normal\" }}{comma}"
                    )?;
                } else {
                    writeln!(
                        file,
                        "        {{ \"name\": \"{}\", \"super\": \"{}\", \"class_type\": \"normal\" }}{}",
                        name, self.get_class_name(*sup), comma
                    )?;
                }
            } else if let Some(UnimplementedClass { name, .. }) = host_obj.as_any().downcast_ref() {
                writeln!(
                    file,
                    "        {{ \"name\": \"{name}\", \"class_type\": \"unimplemented\" }}{comma}"
                )?;
            } else if let Some(FakeClass { name, .. }) = host_obj.as_any().downcast_ref() {
                writeln!(
                    file,
                    "        {{ \"name\": \"{name}\", \"class_type\": \"fake\" }}{comma}"
                )?;
            } else {
                panic!("Unrecognized class type!");
            }
        }
        writeln!(file, "    ]\n}}")
    }

    /// For use by [crate::dyld]: register all the categories from the
    /// application binary.
    pub fn register_bin_categories(&mut self, bin: &MachO, mem: &mut Mem) {
        let Some(list) = bin.get_section("__objc_catlist") else {
            return;
        };

        assert!(list.size % 4 == 0);
        let base: ConstPtr<ConstPtr<category_t>> = Ptr::from_bits(list.addr);
        for i in 0..(list.size / 4) {
            let cat_ptr = mem.read(base + i);
            let data = mem.read(cat_ptr);

            let name = mem.cstr_at_utf8(data.name).unwrap();
            let class = data.class;
            let metaclass = Self::read_isa(class, mem);

            for (class, methods) in [
                (class, data.instance_methods),
                (metaclass, data.class_methods),
            ] {
                if methods.is_null() {
                    continue;
                }

                let any = self.get_host_object(class).unwrap().as_any();
                if any.is::<FakeClass>() || any.is::<UnimplementedClass>() {
                    continue;
                }

                // Horrible workaround to avoid double-borrowing self:
                // temporarily replace the class object.
                let mut host_obj = std::mem::replace(
                    self.borrow_mut::<ClassHostObject>(class),
                    ClassHostObject {
                        name: Default::default(),
                        is_metaclass: Default::default(),
                        superclass: nil,
                        base_protocols: Ptr::null(),
                        methods: Default::default(),
                        guest_method_signatures: Default::default(),
                        instance_start: Default::default(),
                        instance_size: Default::default(),
                        ivars: Default::default(),
                        properties: Default::default(),
                        is_initialized: InitializationStatus::NotInitialized,
                    },
                );
                log_dbg!(
                    "Adding {} methods from guest app category \"{}\" {:?} to {} \"{}\" {:?}",
                    if host_obj.is_metaclass {
                        "class"
                    } else {
                        "instance"
                    },
                    name,
                    cat_ptr,
                    if host_obj.is_metaclass {
                        "metaclass"
                    } else {
                        "class"
                    },
                    host_obj.name,
                    class,
                );
                host_obj.add_methods_from_bin(methods, mem, self);
                *self.borrow_mut::<ClassHostObject>(class) = host_obj;
            }
        }
    }

    pub fn class_get_superclass(&self, class: Class) -> Class {
        if class == nil {
            nil
        } else {
            self.borrow::<ClassHostObject>(class).superclass
        }
    }

    /// Whether `class` or any of its superclasses adopts the protocol named
    /// `protocol_name`.
    ///
    /// Only classes that came from an app binary can adopt anything: tapHLE's
    /// own classes are built from templates that carry no protocol list, so
    /// they answer `false`. That is a real limit rather than an oversight — a
    /// host class genuinely has no declared conformance to consult — and it is
    /// why this returns what it found rather than a guess.
    pub fn class_conforms_to_protocol(&self, mem: &Mem, class: Class, protocol_name: &str) -> bool {
        let mut class = class;
        while class != nil {
            let &ClassHostObject {
                base_protocols,
                superclass,
                ..
            } = self.borrow(class);
            if protocol_list_contains(mem, base_protocols, protocol_name, 0) {
                return true;
            }
            class = superclass;
        }
        false
    }

    pub fn class_is_subclass_of(&self, class: Class, superclass: Class) -> bool {
        if class == superclass {
            return true;
        }

        let mut class = class;
        loop {
            let &ClassHostObject {
                superclass: next, ..
            } = self.borrow(class);
            if next == nil {
                return false;
            } else if next == superclass {
                return true;
            } else {
                class = next;
            }
        }
    }

    pub fn get_class_name(&self, class: Class) -> &str {
        self.try_get_class_name(class)
            .expect("Could not get class name!")
    }

    pub fn get_superclass(&self, class: Class) -> Class {
        let &ClassHostObject { superclass, .. } = self.borrow(class);
        superclass
    }

    pub fn try_get_class_name(&self, class: Class) -> Option<&str> {
        let host_object = self.get_host_object(class)?;
        if let Some(ClassHostObject { name, .. }) = host_object.as_any().downcast_ref() {
            Some(name)
        } else if let Some(UnimplementedClass { name, .. }) = host_object.as_any().downcast_ref() {
            Some(name)
        } else if let Some(FakeClass { name, .. }) = host_object.as_any().downcast_ref() {
            Some(name)
        } else {
            None
        }
    }

    pub fn is_unimplemented_class(&self, class: Class) -> bool {
        if class == nil {
            return false;
        }
        let host_object = self.get_host_object(class).unwrap();
        matches!(
            host_object.as_any().downcast_ref(),
            Some(UnimplementedClass { .. })
        )
    }

    pub fn is_fake_class(&self, class: Class) -> bool {
        if class == nil {
            return false;
        }
        let host_object = self.get_host_object(class).unwrap();
        matches!(host_object.as_any().downcast_ref(), Some(FakeClass { .. }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An [IvarInfo] for reconciliation tests, which never look at the
    /// descriptor pointer.
    fn test_ivar(offset: ConstPtr<GuestUSize>, alignment: u32) -> IvarInfo {
        IvarInfo {
            descriptor: Ptr::null(),
            offset,
            alignment,
        }
    }

    #[test]
    fn ivar_reconciliation_updates_offset_values_and_preserves_alignment() {
        let mut mem = Mem::new();
        mem.set_null_segment_size(0x1000);

        let first_offset: MutPtr<GuestUSize> = Ptr::from_bits(0x1000);
        let second_offset = first_offset + 1;
        mem.write(first_offset, 4);
        mem.write(second_offset, 8);

        let mut ivars = HashMap::new();
        ivars.insert("first".to_string(), test_ivar(first_offset.cast_const(), 2));
        ivars.insert(
            "second".to_string(),
            test_ivar(second_offset.cast_const(), 3),
        );

        // Raw alignment values are log2 encoded, so a five-byte displacement
        // must round up to eight before the stored values move.
        let diff = reconcile_ivar_offsets(&mut mem, &ivars, 5);

        assert_eq!(diff, 8);
        assert_eq!(mem.read(first_offset), 12);
        assert_eq!(mem.read(second_offset), 16);
        assert_eq!(ivars["first"].offset, first_offset.cast_const());
        assert_eq!(ivars["second"].offset, second_offset.cast_const());
    }

    #[test]
    fn ivar_reconciliation_treats_max_alignment_as_word_size() {
        let mut mem = Mem::new();
        mem.set_null_segment_size(0x1000);

        let offset: MutPtr<GuestUSize> = Ptr::from_bits(0x1000);
        mem.write(offset, 20);

        let mut ivars = HashMap::new();
        // u32::MAX encodes "guest word size" (4 bytes here), matching Apple's
        // ivar_t::alignment(), rather than 1 << u32::MAX.
        ivars.insert("word".to_string(), test_ivar(offset.cast_const(), u32::MAX));

        let diff = reconcile_ivar_offsets(&mut mem, &ivars, 5);

        // Word-size alignment rounds the five-byte displacement up to eight.
        assert_eq!(diff, 8);
        assert_eq!(mem.read(offset), 28);
    }

    #[test]
    fn ivar_reconciliation_skips_anonymous_bitfields() {
        let mut mem = Mem::new();
        mem.set_null_segment_size(0x1000);

        let real_offset: MutPtr<GuestUSize> = Ptr::from_bits(0x1000);
        mem.write(real_offset, 4);

        let null_offset: ConstPtr<GuestUSize> = Ptr::from_bits(0);
        let mut ivars = HashMap::new();
        // A null offset pointer marks an anonymous bitfield: it must be skipped
        // in both the alignment scan and the rewrite, so its large raw
        // alignment does not inflate the displacement and its null pointer is
        // never dereferenced.
        ivars.insert("bitfield".to_string(), test_ivar(null_offset, 31));
        ivars.insert("real".to_string(), test_ivar(real_offset.cast_const(), 0));

        let diff = reconcile_ivar_offsets(&mut mem, &ivars, 3);

        // Only the real ivar (alignment 1) participates, so the displacement
        // stays three instead of being rounded up by the skipped bitfield.
        assert_eq!(diff, 3);
        assert_eq!(mem.read(real_offset), 7);
    }
}

/// `objc_getClass` — the class with this name, or nil.
///
/// Apple's runtime answers nil for a name it does not know (after consulting
/// the class handler, which nothing here installs), and apps rely on that: it
/// is how they probe for a class from a newer OS. Aborting instead turned a
/// successful negative answer into a dead app.
///
/// The name is still logged, because "app asked for a class tapHLE does not
/// implement" is exactly the signal that says what to implement next — and if
/// the app was not probing, the nil will surface shortly afterwards as an
/// unexpectedly inert message send, which this log explains.
/// `objc_allocateClassPair` — create a class and its metaclass at run time.
///
/// Frameworks that generate delegates or proxies build their classes this way,
/// so the app has no compiled class for the runtime to find and the pair must
/// be made on demand. The result is deliberately *not* in the class registry
/// yet: `objc_registerClassPair` publishes it, and between the two calls the
/// caller adds methods and ivars.
///
/// The metaclass links exactly as a compiled one does — a class's isa is its
/// metaclass, and a metaclass's superclass is the superclass's metaclass — so
/// class methods resolve up the chain the same way instance methods do.
pub(super) fn objc_allocateClassPair(
    env: &mut Environment,
    superclass: Class,
    name: ConstPtr<u8>,
    extra_bytes: GuestUSize,
) -> Class {
    let Ok(name) = env.mem.cstr_at_utf8(name) else {
        return nil;
    };
    let name = name.to_string();

    // Creating a class that already exists is an error, not a redefinition.
    if env.objc.get_class(&name, false, &env.mem).is_some() {
        log!(
            "objc_allocateClassPair({:?}) -> nil: that class already exists",
            name
        );
        return nil;
    }
    if extra_bytes != 0 {
        log!(
            "TODO: objc_allocateClassPair({:?}) asked for {} extra bytes, which are not allocated",
            name,
            extra_bytes
        );
    }

    // A root class (nil superclass) would need its metaclass to point at
    // itself, which nothing generating classes at run time actually does.
    if superclass == nil {
        log!(
            "TODO: objc_allocateClassPair({:?}) with no superclass is not supported",
            name
        );
        return nil;
    }

    let superclass_metaclass = ObjC::read_isa(superclass, &env.mem);
    let instance_size = env.objc.borrow::<ClassHostObject>(superclass).instance_size;

    let make = |is_metaclass: bool, superclass: Class, instance_size: GuestUSize| ClassHostObject {
        name: name.clone(),
        is_metaclass,
        superclass,
        // A class created at run time adopts nothing until something declares
        // it does, and nothing here can.
        base_protocols: Ptr::null(),
        methods: HashMap::new(),
        guest_method_signatures: HashMap::new(),
        ivars: HashMap::new(),
        properties: Vec::new(),
        instance_start: instance_size,
        instance_size,
        // Nothing was compiled for this class, so there is no +initialize to
        // run and nothing to wait for.
        is_initialized: InitializationStatus::Initialized,
    };

    let metaclass_object = Box::new(make(
        true,
        superclass_metaclass,
        guest_size_of::<objc_object>(),
    ));
    let class_object = Box::new(make(false, superclass, instance_size));

    let root_metaclass = ObjC::read_isa(superclass_metaclass, &env.mem);
    let metaclass = env
        .objc
        .alloc_static_object(root_metaclass, metaclass_object, &mut env.mem);
    env.objc
        .alloc_static_object(metaclass, class_object, &mut env.mem)
}

/// `objc_registerClassPair` — publish a class built by `objc_allocateClassPair`
/// so that `objc_getClass` and message sends can find it by name.
pub(super) fn objc_registerClassPair(env: &mut Environment, class: Class) {
    if class == nil {
        return;
    }
    let name = env.objc.borrow::<ClassHostObject>(class).name.clone();
    env.objc.classes.insert(name, class);
}

/// `objc_disposeClassPair` — discard a class that was allocated but is no
/// longer wanted. Only the registry entry is removed: the class object itself
/// is static-lifetime, and anything still holding an instance of it would be
/// left with a dangling isa if it were freed.
pub(super) fn objc_disposeClassPair(env: &mut Environment, class: Class) {
    if class == nil {
        return;
    }
    let name = env.objc.borrow::<ClassHostObject>(class).name.clone();
    env.objc.classes.remove(&name);
}

pub(super) fn objc_getClass(env: &mut Environment, name: ConstPtr<u8>) -> id {
    let Ok(name_str) = env.mem.cstr_at_utf8(name) else {
        return nil;
    };
    match env.objc.get_class(name_str, false, &env.mem) {
        Some(class) => class,
        None => {
            log!(
                "objc_getClass({:?}) -> nil: no such class in tapHLE",
                name_str
            );
            nil
        }
    }
}

/// `objc_getMetaClass` — the metaclass of the named class.
///
/// A class object's `isa` is its metaclass, so this is `objc_getClass` followed
/// by one hop. Apps use it to add or inspect class methods, which live on the
/// metaclass rather than the class.
pub(super) fn objc_getMetaClass(env: &mut Environment, name: ConstPtr<u8>) -> id {
    let Ok(name_str) = env.mem.cstr_at_utf8(name) else {
        return nil;
    };
    match env.objc.get_class(name_str, true, &env.mem) {
        Some(metaclass) => metaclass,
        None => {
            log!(
                "objc_getMetaClass({:?}) -> nil: no such class in tapHLE",
                name_str
            );
            nil
        }
    }
}

/// `objc_getClassList` — fill `buffer` with up to `buffer_count` registered
/// classes and return how many there are in total.
///
/// The two-call protocol is the point: a caller passes null first to learn the
/// count, allocates, then calls again. Returning the true total even when the
/// buffer is smaller is what makes that work, so the count is not clamped.
///
/// **The list is what tapHLE has registered, which is not every class the
/// system would have.** Every class defined by the app's own binaries is there,
/// registered when the image loads. tapHLE's own framework classes appear only
/// once something has caused them to be materialised. An app enumerating
/// classes is almost always looking for its own subclasses of something, which
/// this answers correctly; an app looking for framework classes it never
/// mentions will see fewer than a device would.
pub(super) fn objc_getClassList(
    env: &mut Environment,
    buffer: MutPtr<Class>,
    buffer_count: i32,
) -> i32 {
    // Sorted for the same reason class_copyMethodList sorts: the registry is a
    // hash map, so an unsorted answer would differ between runs of the same
    // app.
    let mut names: Vec<&String> = env.objc.classes.keys().collect();
    names.sort();
    let total = names.len() as i32;
    if buffer.is_null() || buffer_count <= 0 {
        return total;
    }
    let classes: Vec<Class> = names
        .into_iter()
        .take(buffer_count as usize)
        .map(|name| env.objc.classes[name])
        .collect();
    for (index, class) in classes.into_iter().enumerate() {
        env.mem.write(buffer + index as GuestUSize, class);
    }
    total
}

/// `objc_lookUpClass` — the same lookup as `objc_getClass`, except that a class
/// the runtime does not know is reported as nil instead of being an error.
///
/// That difference is the entire reason apps call it: it is how you ask whether
/// an optional class exists before using it, which is exactly what code
/// guarding a newer-OS feature does. Treating an unknown name as fatal here
/// would break the check it was written to perform.
pub(super) fn objc_lookUpClass(env: &mut Environment, name: ConstPtr<u8>) -> id {
    let Ok(name_str) = env.mem.cstr_at_utf8(name) else {
        return nil;
    };
    env.objc.get_class(name_str, false, &env.mem).unwrap_or(nil)
}

pub(super) fn class_getSuperclass(env: &mut Environment, cls: Class) -> Class {
    env.objc.class_get_superclass(cls)
}

pub(super) fn class_getInstanceSize(env: &mut Environment, cls: Class) -> GuestUSize {
    if cls == nil {
        0
    } else {
        env.objc.borrow::<ClassHostObject>(cls).instance_size
    }
}

/// Look up a declared property by name, searching the superclass chain as the
/// real runtime does.
///
/// Only classes read from the app binary have property metadata: tapHLE's own
/// host classes are declared with `objc_classes!` and have no property table,
/// so a lookup on one still finds nothing. That is the honest answer — a host
/// class's accessors are methods, not declared properties — but it does mean a
/// caller cannot use this to discover, say, `-[UIView frame]`.
pub(super) fn class_getProperty(
    env: &mut Environment,
    cls: Class,
    name: ConstPtr<u8>,
) -> objc_property_t {
    if cls == nil || name.is_null() {
        return Ptr::null();
    }
    let name = env.mem.cstr_at_utf8(name).unwrap().to_string();

    let mut class = cls;
    loop {
        let Some(&ClassHostObject {
            superclass,
            ref properties,
            ..
        }) = env
            .objc
            .get_host_object(class)
            .and_then(|host_object| host_object.as_any().downcast_ref())
        else {
            return Ptr::null();
        };
        if let Some(&(_, property)) = properties.iter().find(|(n, _)| *n == name) {
            return property;
        }
        if superclass == nil {
            return Ptr::null();
        }
        class = superclass;
    }
}

/// Return a malloc-compatible property array like Apple's runtime does.
///
/// Inherited properties are not included, matching the real runtime: a caller
/// that wants them walks the superclass chain itself. The array is allocated
/// out of the guest heap, so the caller frees it with `free()` as documented.
pub(super) fn class_copyPropertyList(
    env: &mut Environment,
    cls: Class,
    out_count: MutPtr<u32>,
) -> MutPtr<objc_property_t> {
    let properties: Vec<objc_property_t> = match env
        .objc
        .get_host_object(cls)
        .and_then(|host_object| host_object.as_any().downcast_ref::<ClassHostObject>())
    {
        Some(host_object) => host_object.properties.iter().map(|&(_, p)| p).collect(),
        None => Vec::new(),
    };

    if !out_count.is_null() {
        env.mem
            .write(out_count, properties.len().try_into().unwrap());
    }
    if properties.is_empty() {
        // Apple's runtime returns NULL, not an empty allocation, for a class
        // with no properties, and callers check for it before freeing.
        return Ptr::null();
    }

    let size = guest_size_of::<objc_property_t>() * properties.len() as GuestUSize;
    let array: MutPtr<objc_property_t> = env.mem.alloc(size).cast();
    for (i, property) in properties.into_iter().enumerate() {
        env.mem.write(array + i as GuestUSize, property);
    }
    array
}

/// The name a property was declared with.
pub(super) fn property_getName(env: &mut Environment, property: objc_property_t) -> ConstPtr<u8> {
    if property.is_null() {
        return Ptr::null();
    }
    env.mem.read(property).name
}

/// The property's attribute string, e.g. `T@"NSString",&,N,V_title`.
///
/// This is read straight out of the binary, so it describes the property
/// exactly as the app's compiler encoded it — including the getter and setter
/// names a caller needs in order to actually use the property.
pub(super) fn property_getAttributes(
    env: &mut Environment,
    property: objc_property_t,
) -> ConstPtr<u8> {
    if property.is_null() {
        return Ptr::null();
    }
    env.mem.read(property).attributes
}

/// The class's name, as a C string in guest memory.
///
/// The string is made once per class and kept, because a class stores its name
/// as a Rust `String` and the caller wants a `const char*` that stays valid.
/// Apps log class names, sometimes every frame, so allocating per call would
/// leak steadily.
pub(super) fn class_getName(env: &mut Environment, cls: Class) -> ConstPtr<u8> {
    // Apple's runtime answers "nil" for a null class rather than crashing, and
    // code that prints a class name is often doing so precisely because it is
    // not sure it has one.
    if cls == nil {
        if let Some(&existing) = env.objc.class_name_strings.get(&nil) {
            return existing;
        }
        let ptr = env.mem.alloc_and_write_cstr(b"nil").cast_const();
        env.objc.class_name_strings.insert(nil, ptr);
        return ptr;
    }

    if let Some(&existing) = env.objc.class_name_strings.get(&cls) {
        return existing;
    }
    let name = env.objc.get_class_name(cls).to_string();
    let ptr = env.mem.alloc_and_write_cstr(name.as_bytes()).cast_const();
    env.objc.class_name_strings.insert(cls, ptr);
    ptr
}

/// Whether this class object is a metaclass.
pub(super) fn class_isMetaClass(env: &mut Environment, cls: Class) -> bool {
    if cls == nil {
        return false;
    }
    env.objc.borrow::<ClassHostObject>(cls).is_metaclass
}

/// Whether instances of the class respond to a selector.
///
/// This asks the same lookup `objc_msgSend` uses, so a class answering yes here
/// really can be sent that message.
pub(super) fn class_respondsToSelector(env: &mut Environment, cls: Class, sel: SEL) -> bool {
    if cls == nil || sel.is_null() {
        return false;
    }
    env.objc.class_defining_method(cls, sel).is_some()
}
