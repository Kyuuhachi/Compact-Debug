#![feature(fmt_helpers_for_derive)]
//! `{:#?}` formatting, and the `dbg!()` macro, sound nice on paper. But once you try using them...
//!
//! ```text
//! Goto(
//!     Address(
//!         30016,
//!     ),
//! ),
//! Label(
//!     Address(
//!         29990,
//!     ),
//! ),
//! Expr(
//!     Expr(
//!         Expr(
//!             [
//!                 Var(
//!                     0,
//!                 ),
//!                 Const(
//!                     0,
//!                 ),
//!                 Op(
//!                     Ne,
//!                 ),
//!             ],
//!         ),
//!     ),
//!     Address(
//!         30016,
//!     ),
//! ),
//! ```
//!
//! Your dreams of nice and readable output are shattered by a chunk of output more porous than cotton
//! candy, with approximately two tokens of content on each line. Screenful upon screenful of vacuous
//! output for even a moderately complex type. Upset, you reluctantly replace your derived `Debug`
//! implementation with a manual one that eschews `DebugTuple` in favor of `write_str`. However, this
//! results in a catastrophic amount of boilerplate code, and doesn't affect types outside of your
//! control, like the ubiquitous `Option`.
//!
//! That's where this crate comes in. It monkey-patches the pretty-printing machinery so that
//! `DebugTuple` is printed on a single line regardless of `#` flag. The above snippet is printed as:
//!
//! ```text
//! Goto(Address(30016)),
//! Label(Address(29990)),
//! Expr(Expr(Expr([
//!     Var(0),
//!     Const(0),
//!     Op(Ne),
//! ])), Address(30016)),
//! ```
//!
//! In addition, it also patches slices of primitive types, `&str`, and `String` to print on a single line.
//!
//! This crate currently only supports x86_64 architecture, and requires nightly.

use std::sync::{LazyLock, OnceLock};

#[cfg(not(target_arch = "x86_64"))]
compile_error!("only supported on x86_64");

struct Pos(*const u8);
unsafe impl Send for Pos {}
unsafe impl Sync for Pos {}
static MATCHES: OnceLock<Vec<Pos>> = OnceLock::new();

macro_rules! patch_slice {
	($ty:ty, $on:expr, $check:literal) => { {
		static DETOUR: LazyLock<retour::RawDetour> = LazyLock::new(|| unsafe {
			type Hook = for<'a, 'b, 'c> fn(&'a [$ty], &'b mut std::fmt::Formatter<'c>) -> std::fmt::Result;
			#[cfg($check)]
			#[expect(unused)]
			static ORIG_FUNC: Hook = <[$ty] as std::fmt::Debug>::fmt;
			static HOOK: Hook = |elf, f| {
				write!(f, "[")?;
				for (i, v) in elf.iter().enumerate() {
					if i != 0 {
						write!(f, ", ")?;
					}
					std::fmt::Debug::fmt(v, f)?;
				}
				write!(f, "]")
			};
			retour::RawDetour::new(<[$ty] as std::fmt::Debug>::fmt as *const (), HOOK as *const ()).unwrap()
		});
		if $on {
			DETOUR.enable().unwrap();
		} else {
			DETOUR.disable().unwrap();
		}
	} }
}

/// Enables or disables the patch.
///
/// # Panics
/// Panics if the function does not look like expected, which is most likely to happen if `std`
/// changes something internally, or if the compiler finds a better way to optimize it.
///
/// # Safety
/// Aside from the whole concept being inherently unsafe, this will probably have unexpected
/// consequences if called in multi-threaded contexts.
pub unsafe fn enable(on: bool) {
	unsafe {
		let matches = MATCHES.get_or_init(find_all);

		for Pos(ptr) in matches {
			let ptr = ptr.cast_mut();
			let _prot = region::protect_with_handle(ptr, 1, region::Protection::READ_WRITE_EXECUTE)
				.unwrap();
			ptr.write(if on { 0 } else { 0x80 });
		}

		patch_slice!(u8, on, true);
		patch_slice!(u16, on, true);
		patch_slice!(u32, on, true);
		patch_slice!(u64, on, true);
		patch_slice!(u128, on, true);
		patch_slice!(usize, on, true);
		patch_slice!(i8, on, true);
		patch_slice!(i16, on, true);
		patch_slice!(i32, on, true);
		patch_slice!(i64, on, true);
		patch_slice!(i128, on, true);
		patch_slice!(isize, on, true);
		patch_slice!(f32, on, true);
		patch_slice!(f64, on, true);
		patch_slice!(bool, on, true);
		patch_slice!(char, on, true);
		patch_slice!(&str, on, false); // can't get the lifetimes to work out, so skipping that check
		patch_slice!(String, on, true);
	}
}

fn find_all() -> Vec<Pos> {
	unsafe {
		let mut out = Vec::new();
		macro_rules! find {
			($name:path) => {
				do_find(&mut out, stringify!($name), $name as *const () as *const u8);
			};
		}
		find!(std::fmt::DebugTuple::field);
		find!(std::fmt::DebugTuple::finish);
		find!(std::fmt::DebugTuple::finish_non_exhaustive);
		find!(std::fmt::Formatter::debug_tuple_field1_finish);
		find!(std::fmt::Formatter::debug_tuple_field2_finish);
		find!(std::fmt::Formatter::debug_tuple_field3_finish);
		find!(std::fmt::Formatter::debug_tuple_field4_finish);
		find!(std::fmt::Formatter::debug_tuple_field5_finish);
		find!(std::fmt::Formatter::debug_tuple_fields_finish);

		// Check that all the field offsets are the same
		assert!(out.iter().all(|x| x.0 == out[0].0), "field offsets differ");

		out.into_iter().map(|x| Pos(x.1)).collect()
	}
}

// Find pattern f6 4x ?? 04, and record the value of the ?? and the position of the 04.
// End when we find a C3 (ret) or CC (int3) on a position that ends with 0xF.
unsafe fn do_find(out: &mut Vec<(u8, *const u8)>, name: &str, mut ptr: *const u8) {
	let n = out.len();
	loop {
		if (ptr as usize & 0xF) == 0xF && (*ptr == 0xC3 || *ptr == 0xCC) {
			break;
		}
		if *ptr == 0xF6 && *ptr.add(1) & 0xF0 == 0x40 && *ptr.add(3) == 0x80 {
			out.push((*ptr.add(2), ptr.add(3)));
		}
		ptr = ptr.add(1);
	}
	assert!(out.len() > n, "no matches found for {name}");
}

// NOTE: there must only be one test, since enable() is thread-unsafe and global.

#[test]
fn test() {
	#[derive(Debug)]
	#[allow(dead_code)]
	struct A(u32, u32);

	#[allow(dead_code)]
	#[derive(Debug)]
	struct B {
		x: u32,
		y: u32,
	}

	#[allow(dead_code)]
	#[derive(Debug)]
	enum Enum {
		A,
		B(u32),
		C(u32, u32),
		D(u32, u32, u32),
		E(u32, u32, u32, u32),
		F(u32, u32, u32, u32, u32),
	}

	let a = A(8, 32);
	let b = B { x: 8, y: 32 };

	assert_eq!(format!("{a:?}"), "A(8, 32)");
	assert_eq!(format!("{a:#?}"), "A(\n    8,\n    32,\n)");
	assert_eq!(format!("{b:?}"), "B { x: 8, y: 32 }");
	assert_eq!(format!("{b:#?}"), "B {\n    x: 8,\n    y: 32,\n}");

	unsafe { enable(true) };

	assert_eq!(format!("{a:?}"), "A(8, 32)");
	assert_eq!(format!("{a:#?}"), "A(8, 32)");
	assert_eq!(format!("{b:?}"), "B { x: 8, y: 32 }");
	assert_eq!(format!("{b:#?}"), "B {\n    x: 8,\n    y: 32,\n}");

	assert!(!format!("{:#?}", Enum::A).contains('\n'));
	assert!(!format!("{:#?}", Enum::B(0)).contains('\n'));
	assert!(!format!("{:#?}", Enum::C(0, 0)).contains('\n'));
	assert!(!format!("{:#?}", Enum::D(0, 0, 0)).contains('\n'));
	assert!(!format!("{:#?}", Enum::E(0, 0, 0, 0)).contains('\n'));
	assert!(!format!("{:#?}", Enum::F(0, 0, 0, 0, 0)).contains('\n'));

	unsafe { enable(false) };

	assert_eq!(format!("{a:?}"), "A(8, 32)");
	assert_eq!(format!("{a:#?}"), "A(\n    8,\n    32,\n)");
	assert_eq!(format!("{b:?}"), "B { x: 8, y: 32 }");
	assert_eq!(format!("{b:#?}"), "B {\n    x: 8,\n    y: 32,\n}");

	test_list([1u8, 2, 3, 4, 5], "[\n    1,\n    2,\n    3,\n    4,\n    5,\n]", "[1, 2, 3, 4, 5]");
	test_list([123u16], "[\n    123,\n]", "[123]");
	test_list([123u32, 456], "[\n    123,\n    456,\n]", "[123, 456]");
	test_list([123u64, 456, 789], "[\n    123,\n    456,\n    789,\n]", "[123, 456, 789]");
	test_list([123u128, 456, 789, 101112], "[\n    123,\n    456,\n    789,\n    101112,\n]", "[123, 456, 789, 101112]");
	test_list([1i32, 2, 3, 4, 5], "[\n    1,\n    2,\n    3,\n    4,\n    5,\n]", "[1, 2, 3, 4, 5]");
	test_list(["one", "two", "three", "four", "five"], "[\n    \"one\",\n    \"two\",\n    \"three\",\n    \"four\",\n    \"five\",\n]", "[\"one\", \"two\", \"three\", \"four\", \"five\"]");
	test_list([(1, 2), (3, 4), (5, 6)], "[\n    (\n        1,\n        2,\n    ),\n    (\n        3,\n        4,\n    ),\n    (\n        5,\n        6,\n    ),\n]", "[\n    (1, 2),\n    (3, 4),\n    (5, 6),\n]");
}

#[cfg(test)]
fn test_list<T: std::fmt::Debug + Clone, const N: usize>(value: [T; N], long: &str, short: &str) {
	let array = value;
	let array_ref = &array;
	let slice_ref = &array as &[_];
	let vec = array.to_vec();

	assert_eq!(format!("{array:#?}"), long);
	assert_eq!(format!("{array_ref:#?}"), long);
	assert_eq!(format!("{slice_ref:#?}"), long);
	assert_eq!(format!("{vec:#?}"), long);

	unsafe { enable(true) };

	assert_eq!(format!("{array:#?}"), short);
	assert_eq!(format!("{array_ref:#?}"), short);
	assert_eq!(format!("{slice_ref:#?}"), short);
	assert_eq!(format!("{vec:#?}"), short);

	unsafe { enable(false) };

	assert_eq!(format!("{array:#?}"), long);
	assert_eq!(format!("{array_ref:#?}"), long);
	assert_eq!(format!("{slice_ref:#?}"), long);
	assert_eq!(format!("{vec:#?}"), long);
}
