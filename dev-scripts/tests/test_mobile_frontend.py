# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
import tomllib
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
GUI_MANIFEST = ROOT / "crates" / "gui" / "Cargo.toml"


class SharedMobileFrontendTests(unittest.TestCase):
    def test_gui_is_the_shared_native_library(self):
        manifest = tomllib.loads(GUI_MANIFEST.read_text(encoding="utf-8"))
        self.assertEqual(manifest["lib"]["name"], "tapHLE_gui")
        self.assertEqual(manifest["lib"]["path"], "src/lib.rs")
        self.assertEqual(set(manifest["lib"]["crate-type"]), {"cdylib", "rlib", "staticlib"})


    def test_ios_is_a_thin_host_for_the_shared_rust_frontend(self):
        gui = tomllib.loads(GUI_MANIFEST.read_text(encoding="utf-8"))
        core = tomllib.loads((ROOT / "crates/taphle/Cargo.toml").read_text(encoding="utf-8"))
        build = (ROOT / "platforms/ios/scripts/build-rust.sh").read_text(encoding="utf-8")
        sources = list((ROOT / "platforms/ios/Sources").glob("*"))
        self.assertEqual(gui["features"]["ios"], ["tapHLE/ios"])
        self.assertIn("sdl2/static-link", core["features"]["ios"])
        self.assertIn("--package tapHLE_gui", build)
        self.assertFalse(any(path.suffix == ".swift" for path in sources))
        self.assertFalse(any("SwiftUI" in path.read_text(encoding="utf-8", errors="ignore") for path in sources if path.is_file()))


    def test_ios_host_supplies_sdl_uikit_process_bootstrap(self):
        main = (ROOT / "platforms/ios/Sources/main.m").read_text(encoding="utf-8")
        project = (ROOT / "platforms/ios/TapHLE.xcodeproj/project.pbxproj").read_text(encoding="utf-8")
        gui = (ROOT / "crates/gui/src/lib.rs").read_text(encoding="utf-8")
        self.assertIn("#undef main", main)
        self.assertIn("tapHLE_iOS_main", main)
        self.assertIn("pub extern \"C\" fn tapHLE_iOS_main", gui)
        self.assertNotIn("SDL_uikitappdelegate.m in Sources", project)

    def test_ios_host_links_the_shared_static_library_directly(self):
        config = (ROOT / "platforms/ios/Config/Build.xcconfig").read_text(encoding="utf-8")
        self.assertIn("-ObjC \"$(RUST_LIBRARY)\"", config)
        self.assertIn("libtapHLE_gui.a", config)
        self.assertNotIn("-force_load", config)
        self.assertNotIn("-ltapHLE_gui", config)

    def test_ios_host_exposes_safe_area_to_shared_rust_shell(self):
        main = (ROOT / "platforms/ios/Sources/main.m").read_text(encoding="utf-8")
        shell = (ROOT / "crates/gui/src/shell.rs").read_text(encoding="utf-8")
        self.assertIn("tapHLE_iOS_safe_area", main)
        self.assertIn("tapHLE_iOS_safe_area", shell)
        self.assertIn("safe_area_rect", shell)
        build = (ROOT / "platforms/ios/scripts/build-rust.sh").read_text(encoding="utf-8")
        self.assertIn("-undefined,dynamic_lookup", build)

    def test_ios_packages_the_emulator_runtime_resources(self):
        project = (ROOT / "platforms/ios/TapHLE.xcodeproj/project.pbxproj").read_text(encoding="utf-8")
        for resource in ("dylibs", "fonts", "default_options.txt"):
            with self.subTest(resource=resource):
                self.assertIn(f"{resource} in Resources", project)

    def test_ios_keeps_resources_in_the_bundle_and_user_state_writable(self):
        core_paths = (ROOT / "crates/taphle/src/paths.rs").read_text(encoding="utf-8")
        frontend_storage = (ROOT / "crates/gui/src/platform/storage.rs").read_text(encoding="utf-8")
        self.assertIn("target_os = \"ios\"", core_paths)
        self.assertIn("sdl2::filesystem::pref_path", core_paths)
        self.assertIn("#[cfg(target_os = \"ios\")]", frontend_storage)
        self.assertIn("(data_dir(), Vec::new())", frontend_storage)


    def test_ios_static_sdl_links_its_platform_frameworks(self):
        build = (ROOT / "crates/taphle/build.rs").read_text(encoding="utf-8")
        self.assertIn("== \"ios\"", build)
        ios = build.split("== \"ios\"", 1)[1]
        for framework in (
            "AudioToolbox", "AVFoundation", "CoreAudio", "CoreBluetooth",
            "CoreGraphics", "CoreHaptics", "CoreMotion", "Foundation",
            "GameController", "Metal", "OpenGLES", "QuartzCore", "UIKit",
        ):
            with self.subTest(framework=framework):
                self.assertIn(f"\"{framework}\"", ios)
        self.assertIn("cargo:rustc-link-lib=framework={framework}", ios)

    def test_mobile_settings_and_about_are_full_screen_pages(self):
        mobile = (ROOT / "crates/gui/src/ui/mobile.rs").read_text(encoding="utf-8")
        settings = (ROOT / "crates/gui/src/ui/desktop/settings_dialog.rs").read_text(encoding="utf-8")
        app = (ROOT / "crates/gui/src/app.rs").read_text(encoding="utf-8")
        for screen in ("GlobalSettings", "AppSettings", "About"):
            self.assertIn(f"Screen::{screen}", mobile)
        self.assertIn("show_global_mobile", settings)
        self.assertIn("show_app_mobile", settings)
        mobile_branch = app.split("if form_factor_for(mobile_target", 1)[1].split("egui::TopBottomPanel::top", 1)[0]
        self.assertNotIn("self.show_dialogs", mobile_branch)

    def test_mobile_typography_and_controls_have_mobile_minimums(self):
        mobile = (ROOT / "crates/gui/src/ui/mobile.rs").read_text(encoding="utf-8")
        self.assertIn("pub const BODY_TEXT: f32 = 16.0", mobile)
        self.assertIn("pub const SECONDARY_TEXT: f32 = 14.0", mobile)
        self.assertIn("pub const TOUCH_TARGET: f32 = 48.0", mobile)
        self.assertIn("RichText::new(label).size(BODY_TEXT)", mobile)

    def test_mobile_product_surfaces_are_implemented_in_rust_egui(self):
        mobile = (ROOT / "crates/gui/src/ui/mobile.rs").read_text(encoding="utf-8")
        self.assertNotIn("placeholder(", mobile)
        for renderer in (
            "library_screen(",
            "activity_screen(",
            "settings_screen(",
            "details_screen(",
            "show_app_mobile(",
            "developer_log_screen(",
        ):
            with self.subTest(renderer=renderer):
                self.assertIn(renderer, mobile)


    def test_android_resource_links_resolve(self):
        resources = ROOT / "platforms/android/app/src/main/res"
        for link in sorted(resources.rglob("*.png")):
            with self.subTest(link=link.name):
                self.assertTrue(link.resolve().is_file(), f"broken Android resource link: {link}")


    def test_android_runtime_library_is_staged_beside_the_workspace_output(self):
        build_script = (ROOT / "crates/taphle/build.rs").read_text(encoding="utf-8")
        android_block = build_script.split("libc++_shared.so has to be copied", 1)[1].split("if std::env::var(\"CARGO_CFG_TARGET_OS\").unwrap() == \"windows\"", 1)[0]
        self.assertIn("OUT_DIR", android_block)
        self.assertNotIn("package_root\n                .join(\"target\")", android_block)


    def test_ios_guest_falls_back_when_no_audio_device_exists(self):
        openal = (ROOT / "crates/taphle/src/audio/openal.rs").read_text(encoding="utf-8")
        self.assertIn('#[cfg(target_os = "ios")]', openal)
        self.assertIn('b"No Output\\0"', openal)

    def test_ios_guest_uses_the_full_mobile_drawable(self):
        window = (ROOT / "crates/taphle/src/window.rs").read_text(encoding="utf-8")
        self.assertIn('env::consts::OS == "android" || env::consts::OS == "ios"', window)
        self.assertIn("!Self::mobile_fullscreen()", window)

    def test_compositor_restores_the_native_ios_drawable_before_presenting(self):
        window = (ROOT / "crates/taphle/src/window.rs").read_text(encoding="utf-8")
        composition = (ROOT / "crates/taphle/src/frameworks/core_animation/composition.rs").read_text(encoding="utf-8")
        self.assertIn("host_drawable_bindings", window)
        self.assertIn("BindFramebufferOES(gles11::FRAMEBUFFER_OES, host_drawable.0)", composition)
        self.assertIn("BindRenderbufferOES(gles11::RENDERBUFFER_OES, host_drawable.1)", composition)

    def test_mobile_handoff_leases_and_restores_the_shared_sdl_window(self):
        shell = (ROOT / "crates/gui/src/shell.rs").read_text(encoding="utf-8")
        core = (ROOT / "crates/taphle/src/lib.rs").read_text(encoding="utf-8")
        self.assertIn("AppWindowLease::new", shell)
        self.assertIn("app.hand_over(Some(lease));", shell)
        self.assertIn("window.gl_make_current(&_gl_context)", shell)
        self.assertNotIn("drop(window);", shell)
        self.assertIn("pub fn run_app_in_window", core)


    def test_mobile_scroll_surfaces_have_stable_distinct_ids(self):
        mobile = (ROOT / "crates/gui/src/ui/mobile.rs").read_text(encoding="utf-8")
        self.assertIn("id_salt(\"mobile-library\")", mobile)
        for screen in ("activity", "settings", "details", "developer-log"):
            self.assertIn(f'content_ui(ui, rect, "mobile-{screen}"', mobile)
        for screen in ("global-settings", "app-settings", "about"):
            self.assertIn(f'full_page_ui(ui, body_rect, "mobile-{screen}"', mobile)


    def test_mobile_play_always_uses_in_process_runtime_before_emulator_lookup(self):
        app = (ROOT / "crates/gui/src/app.rs").read_text(encoding="utf-8")
        self.assertIn("cfg!(any(target_os = \"android\", target_os = \"ios\"))", app)
        play = app.split("fn play(&mut self", 1)[1].split("fn open_report", 1)[0]
        self.assertLess(play.index("if self.settings.run_in_process"), play.index("let Some(emulator)"))


    def test_android_sdl_entry_belongs_to_the_shared_frontend(self):
        gui = (ROOT / "crates/gui/src/lib.rs").read_text(encoding="utf-8")
        core = (ROOT / "crates/taphle/src/lib.rs").read_text(encoding="utf-8")
        self.assertIn("pub extern \"C\" fn SDL_main", gui)
        self.assertIn("cfg(any(target_os = \"android\", target_os = \"ios\"))", gui)
        self.assertNotIn("pub extern \"C\" fn SDL_main", core)


    def test_android_feature_uses_the_shared_emulator_without_desktop_static_linking(self):
        gui = tomllib.loads(GUI_MANIFEST.read_text(encoding="utf-8"))
        core = tomllib.loads((ROOT / "crates/taphle/Cargo.toml").read_text(encoding="utf-8"))
        self.assertFalse(gui["dependencies"]["tapHLE"]["default-features"])
        self.assertEqual(gui["features"]["android"], ["tapHLE/android"])
        self.assertNotIn("rfd", gui["dependencies"])
        self.assertIn("sdl2/bundled", core["features"]["android"])
        self.assertIn("tapHLE_openal_soft_wrapper/static", core["features"]["android"])


    def test_android_packages_the_shared_gui_library(self):
        activity = (ROOT / "platforms/android/app/src/main/java/org/taphle/android/MainActivity.java").read_text(encoding="utf-8")
        gradle = (ROOT / "platforms/android/app/build.gradle.kts").read_text(encoding="utf-8")
        settings = (ROOT / "platforms/android/settings.gradle.kts").read_text(encoding="utf-8")
        self.assertIn("extends SDLActivity", activity)
        self.assertIn("nativeSendQuit()", activity)
        self.assertIn("dispatchKeyEvent", activity)
        self.assertIn("KEYCODE_BACK", activity)
        self.assertIn("\"tapHLE_gui\"", activity)
        self.assertIn("module = \"../..\"", gradle)
        self.assertIn("targets = arrayListOf(\"arm64\", \"x86_64\")", gradle)
        self.assertIn("abiFilters(\"arm64-v8a\", \"x86_64\")", gradle)
        self.assertIn("libtapHLE_gui.so", gradle)
        self.assertIn("\"--features\",\n        \"android\"", gradle)
        self.assertIn("\"--package\",\n        \"tapHLE_gui\"", gradle)
        self.assertNotIn("include(\":taphle\")", settings)


if __name__ == "__main__":
    unittest.main()
