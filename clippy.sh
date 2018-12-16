#!/bin/bash

# https://github.com/rust-lang/rust-clippy/issues/2604
touch `find * | grep '\.rs' | grep -v target | xargs`

# TODO Remove all of these exceptions
# TODO Report issues for some of these false positives
cargo clippy -- \
	-A clippy::borrowed_box \
	-A clippy::collapsible_if \
	-A clippy::cyclomatic_complexity \
	-A clippy::expect_fun_call \
	-A clippy::if_same_then_else \
	-A clippy::large_enum_variant \
	-A clippy::map_entry \
	-A clippy::needless_pass_by_value \
	-A clippy::new_ret_no_self \
	-A clippy::new_without_default \
	-A clippy::new_without_default_derive \
	-A clippy::ptr_arg \
	-A clippy::suspicious_arithmetic_impl \
	-A clippy::too_many_arguments \
	-A clippy::type_complexity \
	-A clippy::wrong_self_convention
