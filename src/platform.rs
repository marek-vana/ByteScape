#[derive(Clone, Copy, Default)]
pub struct Preferences {
    pub reduce_motion: bool,
    pub high_contrast: bool,
    pub screen_reader: bool,
}
#[cfg(target_os = "macos")]
pub fn preferences() -> Preferences {
    let workspace = objc2_app_kit::NSWorkspace::sharedWorkspace();
    Preferences {
        reduce_motion: workspace.accessibilityDisplayShouldReduceMotion(),
        high_contrast: workspace.accessibilityDisplayShouldIncreaseContrast(),
        screen_reader: workspace.isVoiceOverEnabled(),
    }
}
#[cfg(not(target_os = "macos"))]
pub fn preferences() -> Preferences {
    Preferences::default()
}

/// NSOpenPanel supplies a native file-system URL. Cancellation has no side effects.
#[cfg(target_os = "macos")]
pub fn choose_folder(cs: bool) -> Option<std::path::PathBuf> {
    use std::{
        ffi::{CStr, OsStr},
        os::unix::ffi::OsStrExt,
    };
    let panel = objc2_app_kit::NSOpenPanel::openPanel(objc2::MainThreadMarker::new()?);
    let title = objc2_foundation::NSString::from_str(if cs {
        "Vybrat složku ke skenování"
    } else {
        "Choose a folder to scan"
    });
    let prompt =
        objc2_foundation::NSString::from_str(if cs { "Spustit sken" } else { "Start scan" });
    panel.setTitle(Some(&title));
    panel.setPrompt(Some(&prompt));
    panel.setCanChooseDirectories(true);
    panel.setCanChooseFiles(false);
    panel.setAllowsMultipleSelection(false);
    panel.setResolvesAliases(false);
    if panel.runModal() != 1 {
        return None;
    }
    let url = panel.URL()?;
    let bytes = unsafe { CStr::from_ptr(url.fileSystemRepresentation().as_ptr()) };
    Some(std::path::PathBuf::from(OsStr::from_bytes(
        bytes.to_bytes(),
    )))
}

/// Dedicated scan threads opt out before touching any path (including ancestors).
/// TN3150; constants from the macOS SDK's sys/resource.h. Failure is fail-closed.
pub fn disable_materialization() -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn setiopolicy_np(iotype: i32, scope: i32, policy: i32) -> i32;
        }
        if unsafe { setiopolicy_np(3, 1, 1) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Ask macOS for its package classification, without opening file contents.
pub fn is_package(path: &std::path::Path) -> std::io::Result<bool> {
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::{NSNumber, NSURL, NSURLIsPackageKey};
        use std::{ffi::CString, os::unix::ffi::OsStrExt, ptr::NonNull};
        objc2::rc::autoreleasepool(|_| {
            let path = CString::new(path.as_os_str().as_bytes())?;
            // NSURL consumes the native path during this call; no lossy conversion.
            let url = unsafe {
                NSURL::fileURLWithFileSystemRepresentation_isDirectory_relativeToURL(
                    NonNull::new(path.as_ptr().cast_mut()).unwrap(),
                    true,
                    None,
                )
            };
            let mut value = None;
            unsafe { url.getResourceValue_forKey_error(&mut value, NSURLIsPackageKey) }
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            value
                .and_then(|v| v.downcast::<NSNumber>().ok())
                .map(|v| v.boolValue())
                .ok_or_else(|| {
                    std::io::Error::other(
                        "Package metadata unavailable / metadata balíčku nedostupná",
                    )
                })
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Ok(false)
    }
}
