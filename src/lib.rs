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
//! This crate currently only supports x86_64 architecture, and requires nightly.

use std::sync::OnceLock;

#[cfg(not(target_arch = "x86_64"))]
compile_error!("only supported on x86_64");

struct Pos(*const u8);
unsafe impl Send for Pos {}
unsafe impl Sync for Pos {}
static MATCHES: OnceLock<Vec<Pos>> = OnceLock::new();


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
			ptr.write(if on { 0 } else { 4 });
		}
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
		if *ptr == 0xF6 && *ptr.add(1) & 0xF0 == 0x40 && *ptr.add(3) == 0x04 {
			out.push((*ptr.add(2), ptr.add(3)));
		}
		ptr = ptr.add(1);
	}
	assert!(out.len() > n, "no matches found for {name}");
}

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

	unsafe { enable(false) };

	assert_eq!(format!("{a:?}"), "A(8, 32)");
	assert_eq!(format!("{a:#?}"), "A(\n    8,\n    32,\n)");
	assert_eq!(format!("{b:?}"), "B { x: 8, y: 32 }");
	assert_eq!(format!("{b:#?}"), "B {\n    x: 8,\n    y: 32,\n}");
}
