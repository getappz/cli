use env_var::GlobalEnvBag;
use starbase_styles::color::{no_color, supports_color};
use std::env;
use std::ffi::OsString;

pub fn is_arg_executable(arg: &str) -> bool {
    arg.ends_with("appz") || arg.ends_with("appz.exe")
}

pub fn gather_args() -> (Vec<OsString>, bool) {
    let mut args: Vec<OsString> = vec![];
    let mut leading_args: Vec<OsString> = vec![];
    let mut check_for_target = true;
    let mut has_executable = false;

    env::args_os().enumerate().for_each(|(index, arg)| {
        if let Some(a) = arg.to_str() {
            // Script being executed, so persist it
            if index == 0 && is_arg_executable(a) {
                leading_args.push(arg);
                has_executable = true;
                return;
            }

            // Find first non-option value
            if check_for_target && !a.starts_with('-') {
                check_for_target = false;
            }
        }

        args.push(arg);
    });

    leading_args.extend(args);

    (leading_args, has_executable)
}

pub fn setup_colors(force_color: bool) {
    let bag = GlobalEnvBag::instance();

    if force_color {
        bag.set("FORCE_COLOR", "1");
        bag.set("CLICOLOR", "1");
        bag.set("CLICOLOR_FORCE", "1");
    } else if no_color() {
        setup_no_colors();
    } else if supports_color() == 0 {
        bag.set("CLICOLOR", "0");
    } else {
        // Default: enable colors
        bag.set("CLICOLOR", "1");
    }
}

pub fn setup_no_colors() {
    let bag = GlobalEnvBag::instance();
    bag.set("NO_COLOR", "1");
    bag.set("CLICOLOR", "0");
    bag.set("CLICOLOR_FORCE", "0");
}
