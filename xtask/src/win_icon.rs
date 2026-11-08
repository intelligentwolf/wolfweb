//! Windows: put WolfWeb's icon into wolfweb.exe.
//!
//! wolfweb.exe is CEF's prebuilt bootstrap.exe, so it carries CEF's icon: Explorer, shortcuts and (through
//! CefSettings.chrome_app_icon_id, src/lib.rs) the taskbar all show it. The bundler replaces the exe's icon
//! resources with resources/win/wolfweb.ico.
//!
//! Source: CEF libcef_dll/bootstrap/win/bootstrap.rc:34 `IDI_APPLICATION ICON "bootstrap.ico"` under
//! `LANGUAGE LANG_ENGLISH, SUBLANG_ENGLISH_US` (:22). Checked in the v0.1.1 release's wolfweb.exe: one RT_GROUP_ICON,
//! id 32512 (winuser.h IDI_APPLICATION), language 1033, 118 bytes for 8 images (6-byte header + 8 × 14-byte
//! entries), and RT_ICON ids 1–8. The exe is unsigned (no security directory), so editing it is safe: CEF's bootstrap
//! accepts an unsigned exe with an unsigned DLL (bootstrap_win.cc:172-174, :931-932).

/// The icon group id. src/lib.rs passes the same number to CEF as chrome_app_icon_id.
#[cfg(target_os = "windows")]
pub const WIN_ICON_GROUP_ID: u16 = 32512;
/// LANG_ENGLISH / SUBLANG_ENGLISH_US = MAKELANGID(0x09, 0x01), the language bootstrap.rc gives its resources.
#[cfg(target_os = "windows")]
pub const WIN_ICON_LANGUAGE: u16 = 0x0409;

/// One image of a .ico file: its 12-byte directory entry head and its bytes.
///
/// .ico layout: ICONDIR { reserved u16 = 0, type u16 = 1, count u16 } then `count` ICONDIRENTRY { width u8,
/// height u8, colors u8, reserved u8, planes u16, bit_count u16, bytes_in_res u32, image_offset u32 }, little-endian.
/// The resource form (RT_GROUP_ICON) keeps the same 6-byte header and the first 12 bytes of each entry, then a u16
/// RT_ICON id in place of the file offset — the 14-byte entries counted above.
#[derive(Debug, PartialEq, Eq)]
pub struct IcoImage<'a> {
    pub entry_head: [u8; 12],
    pub data: &'a [u8],
}

pub fn parse_ico(ico: &[u8]) -> Result<Vec<IcoImage<'_>>, String> {
    let u16_at = |at: usize| ico.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |at: usize| ico.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    if u16_at(0) != Some(0) || u16_at(2) != Some(1) {
        return Err("not an icon file (bad ICONDIR header)".into());
    }
    let count = u16_at(4).ok_or("truncated icon file")? as usize;
    if count == 0 {
        return Err("icon file has no images".into());
    }
    let mut images = Vec::with_capacity(count);
    for i in 0..count {
        let at = 6 + i * 16;
        let head: [u8; 12] = ico.get(at..at + 12).ok_or("truncated icon directory")?.try_into().expect("12 bytes");
        let len = u32_at(at + 8).ok_or("truncated icon directory")? as usize;
        let offset = u32_at(at + 12).ok_or("truncated icon directory")? as usize;
        let data = offset.checked_add(len).and_then(|end| ico.get(offset..end)).ok_or("icon image outside the file")?;
        images.push(IcoImage { entry_head: head, data });
    }
    Ok(images)
}

/// The RT_GROUP_ICON data for `images`, whose RT_ICON ids are first_id, first_id + 1, …
///
/// Like the resource compiler, a planes or bit-count of 0 in the file is filled in: planes becomes 1, and a bitmap
/// image's bit count comes from its BITMAPINFOHEADER (biBitCount, u16 at byte 14). Checked against CEF's
/// bootstrap.ico and the RT_GROUP_ICON rc.exe built from it in the v0.1.1 wolfweb.exe: its two 16-colour images
/// are `planes 0, bits 0` in the file and `planes 1, bits 4` in the exe; every other byte is copied as is.
pub fn group_icon(images: &[IcoImage<'_>], first_id: u16) -> Vec<u8> {
    let mut out = Vec::with_capacity(6 + images.len() * 14);
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    for (i, image) in images.iter().enumerate() {
        let mut head = image.entry_head;
        if head[4..6] == [0, 0] {
            head[4..6].copy_from_slice(&1u16.to_le_bytes());
        }
        let is_png = image.data.starts_with(b"\x89PNG");
        if head[6..8] == [0, 0] && !is_png {
            if let Some(bits) = image.data.get(14..16) {
                head[6..8].copy_from_slice(bits);
            }
        }
        out.extend_from_slice(&head);
        out.extend_from_slice(&(first_id + i as u16).to_le_bytes());
    }
    out
}

/// Replace every icon in `exe` with the images of `ico`: all RT_GROUP_ICON and RT_ICON resources (any name, any
/// language) are removed, then the images go in as RT_ICON 1…n and one RT_GROUP_ICON WIN_ICON_GROUP_ID.
#[cfg(target_os = "windows")]
pub fn replace_exe_icon(exe: &std::path::Path, ico: &[u8]) -> anyhow::Result<()> {
    use anyhow::{bail, Context};
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::core::{BOOL, PCWSTR};
    use windows_sys::Win32::Foundation::{FreeLibrary, HMODULE};
    use windows_sys::Win32::System::LibraryLoader::{
        BeginUpdateResourceW, EndUpdateResourceW, EnumResourceLanguagesW, EnumResourceNamesW, LoadLibraryExW,
        UpdateResourceW, LOAD_LIBRARY_AS_DATAFILE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{RT_GROUP_ICON, RT_ICON};

    /// A resource name: an integer id, or a string (kept as UTF-16 with its NUL).
    #[derive(Clone)]
    enum Name {
        Id(u16),
        Str(Vec<u16>),
    }
    impl Name {
        // IS_INTRESOURCE: an id is a "pointer" whose high bits are zero (winuser.h MAKEINTRESOURCE).
        unsafe fn from_ptr(p: PCWSTR) -> Name {
            if (p as usize) >> 16 == 0 {
                Name::Id(p as usize as u16)
            } else {
                let mut len = 0;
                while *p.add(len) != 0 {
                    len += 1;
                }
                Name::Str(std::slice::from_raw_parts(p, len + 1).to_vec())
            }
        }
        fn as_ptr(&self) -> PCWSTR {
            match self {
                Name::Id(id) => *id as usize as PCWSTR,
                Name::Str(s) => s.as_ptr(),
            }
        }
    }

    unsafe extern "system" fn on_name(_: HMODULE, _: PCWSTR, name: PCWSTR, found: isize) -> BOOL {
        (*(found as *mut Vec<Name>)).push(Name::from_ptr(name));
        1
    }
    unsafe extern "system" fn on_language(_: HMODULE, _: PCWSTR, _: PCWSTR, language: u16, found: isize) -> BOOL {
        (*(found as *mut Vec<u16>)).push(language);
        1
    }

    let images = parse_ico(ico).map_err(anyhow::Error::msg).context("resources/win/wolfweb.ico")?;
    let wide: Vec<u16> = exe.as_os_str().encode_wide().chain(std::iter::once(0)).collect();

    // The icons the exe has now, as (type, name, language).
    let mut existing: Vec<(PCWSTR, Name, u16)> = Vec::new();
    unsafe {
        let module = LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), LOAD_LIBRARY_AS_DATAFILE);
        if module.is_null() {
            bail!("couldn't open {} to read its icons: {}", exe.display(), std::io::Error::last_os_error());
        }
        for kind in [RT_GROUP_ICON, RT_ICON] {
            let mut names: Vec<Name> = Vec::new();
            // Returns 0 with ERROR_RESOURCE_TYPE_NOT_FOUND when there are none, which leaves `names` empty.
            EnumResourceNamesW(module, kind, Some(on_name), &mut names as *mut _ as isize);
            for name in names {
                let mut languages: Vec<u16> = Vec::new();
                let languages_ptr = &mut languages as *mut Vec<u16> as isize;
                EnumResourceLanguagesW(module, kind, name.as_ptr(), Some(on_language), languages_ptr);
                for language in languages {
                    existing.push((kind, name.clone(), language));
                }
            }
        }
        FreeLibrary(module);
    }

    unsafe {
        let update = BeginUpdateResourceW(wide.as_ptr(), 0);
        if update.is_null() {
            bail!("couldn't open {} to change its icon: {}", exe.display(), std::io::Error::last_os_error());
        }
        // The first failing call's error; later calls are skipped once one fails.
        let mut failed: Option<std::io::Error> = None;
        let mut update_resource = |kind: PCWSTR, name: PCWSTR, language: u16, data: &[u8]| {
            if failed.is_some() {
                return;
            }
            // A null pointer with size 0 deletes the resource (UpdateResourceW).
            let ptr = if data.is_empty() { std::ptr::null() } else { data.as_ptr().cast() };
            if UpdateResourceW(update, kind, name, language, ptr, data.len() as u32) == 0 {
                failed = Some(std::io::Error::last_os_error());
            }
        };
        for (kind, name, language) in &existing {
            update_resource(*kind, name.as_ptr(), *language, &[]);
        }
        for (i, image) in images.iter().enumerate() {
            update_resource(RT_ICON, (1 + i) as u16 as usize as PCWSTR, WIN_ICON_LANGUAGE, image.data);
        }
        let group = group_icon(&images, 1);
        update_resource(RT_GROUP_ICON, WIN_ICON_GROUP_ID as usize as PCWSTR, WIN_ICON_LANGUAGE, &group);
        // On failure, discard (fdiscard = TRUE) so the exe is left as it was.
        let committed = EndUpdateResourceW(update, failed.is_some() as BOOL) != 0;
        if let Some(error) = failed {
            bail!("couldn't change the icon of {}: {error}", exe.display());
        }
        if !committed {
            bail!("couldn't save the icon into {}: {}", exe.display(), std::io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A .ico with two images: 3 bytes at offset 38 and 2 bytes at offset 41.
    fn two_image_ico() -> Vec<u8> {
        let mut ico = vec![0, 0, 1, 0, 2, 0];
        ico.extend_from_slice(&[16, 16, 0, 0, 1, 0, 32, 0, 3, 0, 0, 0, 38, 0, 0, 0]);
        ico.extend_from_slice(&[0, 0, 0, 0, 1, 0, 32, 0, 2, 0, 0, 0, 41, 0, 0, 0]);
        ico.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);
        ico
    }

    #[test]
    fn parses_images_and_builds_the_group() {
        let ico = two_image_ico();
        let images = parse_ico(&ico).unwrap();
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].data, &[0xAA, 0xBB, 0xCC]);
        assert_eq!(images[1].data, &[0xDD, 0xEE]);
        let group = group_icon(&images, 1);
        assert_eq!(group.len(), 6 + 2 * 14);
        assert_eq!(&group[..6], &[0, 0, 1, 0, 2, 0]);
        assert_eq!(&group[6..20], &[16, 16, 0, 0, 1, 0, 32, 0, 3, 0, 0, 0, 1, 0]);
        assert_eq!(&group[20..34], &[0, 0, 0, 0, 1, 0, 32, 0, 2, 0, 0, 0, 2, 0]);
    }

    #[test]
    fn rejects_broken_files() {
        assert!(parse_ico(b"PNG").is_err());
        assert!(parse_ico(&[0, 0, 1, 0, 0, 0]).is_err());
        let mut ico = two_image_ico();
        ico.truncate(40);
        assert!(parse_ico(&ico).is_err());
    }

    #[test]
    fn wolfweb_ico_parses() {
        let ico = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../resources/win/wolfweb.ico")).unwrap();
        let images = parse_ico(&ico).unwrap();
        assert!(!images.is_empty());
        // Every image is a PNG (the .ico is PNG-compressed), and the group fits the 6 + 14 × n layout.
        assert!(images.iter().all(|i| i.data.starts_with(b"\x89PNG")));
        assert_eq!(group_icon(&images, 1).len(), 6 + images.len() * 14);
    }
}
