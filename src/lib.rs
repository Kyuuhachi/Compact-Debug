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
//! This crate currently only supports x86_64 architecture.

use std::sync::OnceLock;

#[cfg(not(target_arch = "x86_64"))]
compile_error!("only supported on x86_64");

#[derive(Debug, Clone, Copy)]
struct Position(usize, [u8; 2]);

const POSITIONS: &[Position] = &[
	Position(0x44, [0x75, 0x42]), // 2023-04-11, 2024-04-20, 2024-07-10, 1.79.0
	Position(0x49, [0x75, 0x43]), // 2023-05-11
	Position(0x46, [0x75, 0x3E]), // 2024-02-06, 1.77.2
];

const NOP: [u8; 2] = [0x66, 0x90];

static WHICH: OnceLock<Position> = OnceLock::new();

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
		let function = std::fmt::DebugTuple::field as *const () as *const u8;
		let pos = WHICH.get_or_init(|| find(function));
		let ptr = function.add(pos.0) as *mut [u8; 2];
		if *ptr != pos.1 && *ptr != NOP {
			panic!("DebugTuple::field is not as expected (is {:02X?})", *ptr);
		}
		let _prot =
			region::protect_with_handle(ptr, 2, region::Protection::READ_WRITE_EXECUTE).unwrap();
		ptr.write(if on { NOP } else { pos.1 });
	}
}

unsafe fn find(ptr: *const u8) -> Position {
	for &pos in POSITIONS {
		let ptr = ptr.add(pos.0) as *const [u8; 2];
		if *ptr == pos.1 {
			return pos;
		}
	}

	#[cfg(test)]
	{
		// If we couldn't find the position, try to guess one (but only print it, don't actually use it)
		#[derive(Debug, Clone, Copy)]
		enum State {
			Start,
			Ret,
		}
		let mut state = State::Start;
		for i in 0..128 {
			match (state, *ptr.add(i)) {
				(State::Start, 0xC3) => state = State::Ret,
				(State::Ret, 0x75) => panic!(
					"could not find position - might be Position({:#02X}, [{:#02X}, {:#02X}])",
					i,
					*ptr.add(i),
					*ptr.add(i + 1)
				),
				_ => (),
			}
		}

		for i in 0..128 {
			print!("{:02X} ", *ptr);
			if i % 32 == 31 {
				println!();
			}
		}
	}

	panic!("could not find position");
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
