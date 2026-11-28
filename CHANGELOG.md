## [1.8.0](https://github.com/yggdrion/valheim-tracker/compare/v1.7.0...v1.8.0) (2026-11-28)

### ✨ Features

* faster app start with a loading spinner until the characters are read ([6694587](https://github.com/yggdrion/valheim-tracker/commit/6694587a192f76995c4ad6118a29446a3f64f0ac))

### 🐛 Bug Fixes

* wider app window and Hall so the Saga headings fit one line, no empty space or scrollbar at the bottom ([99fd226](https://github.com/yggdrion/valheim-tracker/commit/99fd2260dce9f0106dbb49e7e37bf5fe423a5606))

## [1.7.0](https://github.com/yggdrion/valheim-tracker/compare/v1.6.0...v1.7.0) (2026-11-28)

### ✨ Features

* **dashboard:** show when the Hall was last updated, top right ([197546f](https://github.com/yggdrion/valheim-tracker/commit/197546f452825699667db6694410bf513712ca95))

## [1.6.0](https://github.com/yggdrion/valheim-tracker/compare/v1.5.0...v1.6.0) (2026-11-27)

### ✨ Features

* the Local view shows only the characters who have been in the chosen world ([0e4a567](https://github.com/yggdrion/valheim-tracker/commit/0e4a567ae2aa370a554ef5b4376ec7ce0cb3277c))

### 🐛 Bug Fixes

* **dashboard:** keep the tab bar pinned at the top while scrolling ([0100aa2](https://github.com/yggdrion/valheim-tracker/commit/0100aa2be9c84094d0ae74e5512c75471996fb4f))

## [1.5.0](https://github.com/yggdrion/valheim-tracker/compare/v1.4.0...v1.5.0) (2026-11-27)

### ✨ Features

* the Hall in tabs (Saga, The World, The Road, Achievements, Map), no big header ([78f32e4](https://github.com/yggdrion/valheim-tracker/commit/78f32e4a4c891a04920efb7094e751ab910fae6a))

### 📝 Documentation

* the rename shipped in 1.4.0 ([1d5d28a](https://github.com/yggdrion/valheim-tracker/commit/1d5d28a3581f4748fa36089b3c9fd8851d7864fa))

## [1.4.0](https://github.com/yggdrion/valheim-tracker/compare/v1.3.1...v1.4.0) (2026-11-27)

### ✨ Features

* **core:** achievement and saga analysis in Rust ([3f5b3e5](https://github.com/yggdrion/valheim-tracker/commit/3f5b3e5b02983cdf90a0736005d89e6b2bd122dc))
* **core:** character state store compatible with the Python layout ([e8d8615](https://github.com/yggdrion/valheim-tracker/commit/e8d86153cd28470cf596fe00ba3dba4a0f6cee59))
* **core:** explored map, pins and points from saves in Rust ([9ec275e](https://github.com/yggdrion/valheim-tracker/commit/9ec275ef990aaca336607b0491de428f3018d2f3))
* **core:** the Hall's HTTP routes in Rust ([ea8055f](https://github.com/yggdrion/valheim-tracker/commit/ea8055f8dfb91667b07560848f9b6cca91cf3986))
* **core:** Valheim .fch reader in Rust ([83072f0](https://github.com/yggdrion/valheim-tracker/commit/83072f06ce492f7bcebb48f732e4b40e12ef4c7a))
* **core:** world folder reader in Rust ([707de1f](https://github.com/yggdrion/valheim-tracker/commit/707de1f6ffcc49ef611964763945f9969216bebc))
* Python runtime replaced by the Rust core ([f446a57](https://github.com/yggdrion/valheim-tracker/commit/f446a57559c10b2910d405fd1801509767dc1169))
* the app is now called Valheim Tracker (was Tracker Sync) ([ba0f180](https://github.com/yggdrion/valheim-tracker/commit/ba0f18099aa140f6256ba758001cd4b196ece39f))
* the Hall inside Tracker Sync, with Local and Group views ([6d27af4](https://github.com/yggdrion/valheim-tracker/commit/6d27af425275077acc6d8bfefd7dca5cf2edfd39))
* Tracker Sync runs the Hall itself for local play ([b5c0066](https://github.com/yggdrion/valheim-tracker/commit/b5c0066c15a0c3b5979d6692b33708f1e696ce0a))
* tracker-server, the Rust Hall server ([9443c05](https://github.com/yggdrion/valheim-tracker/commit/9443c058b1e8f2b00fdf6ea2490ec8051adb1882))

### 🐛 Bug Fixes

* **core:** Hall world cache survives parser panics and caches only successes ([a19f58c](https://github.com/yggdrion/valheim-tracker/commit/a19f58c759b324b104217ee5a6a9cbf675d49cba))
* **core:** serialise store writes and guard analysis panics ([eac747c](https://github.com/yggdrion/valheim-tracker/commit/eac747c7d59458a033fac14f6c5e4b54f7c4d104))
* **core:** Store::names, float zero explored is empty, Host on Request, world cache keyed by folder ([7236b39](https://github.com/yggdrion/valheim-tracker/commit/7236b39150cc9a193c559610a8b50fc03211eea0))
* **scripts:** tracker-server binary name with .exe on Windows ([c390716](https://github.com/yggdrion/valheim-tracker/commit/c390716eb4ff7fff9db6f59da8aa97b8f85dc006))
* **sync:** no panic or ghost characters when the folder changes mid-tick, local Hall checks Host and keeps its port ([c49d2d3](https://github.com/yggdrion/valheim-tracker/commit/c49d2d3849d2d4f02582ad74f4edbbfeb0d23058))
* **sync:** proxy only origin-form paths, no file I/O under the state lock, keep characters on unreadable folders ([14c0474](https://github.com/yggdrion/valheim-tracker/commit/14c0474b165261dff59bf1db0afebb53ddd04b55))

### 🔧 Chores

* cargo workspace with tracker-core and parity goldens script ([b6745f2](https://github.com/yggdrion/valheim-tracker/commit/b6745f23c504baf9f39eae231e768e350c01e5b3))

### 📝 Documentation

* design for a local-first tracker with one Rust core ([2afc108](https://github.com/yggdrion/valheim-tracker/commit/2afc1085ccf4717ceddf2bc75ec7ba9ea3f0240d))
* implementation plan for the local-first tracker ([b54589c](https://github.com/yggdrion/valheim-tracker/commit/b54589c96bb1fa693ceff15a1401f76c45509efc))

## [1.3.1](https://github.com/yggdrion/valheim-tracker/compare/v1.3.0...v1.3.1) (2026-11-26)

### 🐛 Bug Fixes

* Tracker Sync frameless window with its own title bar, scrolling character list, no checkbox lag ([4414e16](https://github.com/yggdrion/valheim-tracker/commit/4414e16ddfbc0585a33115ed3f2ec74e9337e592))

## [1.3.0](https://github.com/yggdrion/valheim-tracker/compare/v1.2.1...v1.3.0) (2026-11-26)

### ✨ Features

* Tracker Sync updates itself from signed GitHub releases ([4a1f551](https://github.com/yggdrion/valheim-tracker/commit/4a1f551592f0bf7fd59148024b75423e72e7f850))

## [1.2.1](https://github.com/yggdrion/valheim-tracker/compare/v1.2.0...v1.2.1) (2026-11-26)

### 🐛 Bug Fixes

* Tracker Sync opens wide (1000x760) in the two-column layout ([185fa0d](https://github.com/yggdrion/valheim-tracker/commit/185fa0ddeb72438c159f77f07358d27f44d3831c))

### 📝 Documentation

* todo: drop map extras and the optional server items ([56e5165](https://github.com/yggdrion/valheim-tracker/commit/56e516592a4e3d46e5915bd526c8ba6d1e791a38))

## [1.2.0](https://github.com/yggdrion/valheim-tracker/compare/v1.1.0...v1.2.0) (2026-11-26)

### ✨ Features

* own save parser and game data instead of Melichor's achi_hunter.py ([cf64eee](https://github.com/yggdrion/valheim-tracker/commit/cf64eee18bb876a899c1aa9c564325e47762fd16))

### 🔧 Chores

* remove the fchsync scripts, Tracker Sync replaces them ([855a7bc](https://github.com/yggdrion/valheim-tracker/commit/855a7bc76a5d248eef5b1a56e6e7b45ba6bb76ba))

## [1.1.0](https://github.com/yggdrion/valheim-tracker/compare/v1.0.0...v1.1.0) (2026-11-26)

### ✨ Features

* Tracker Sync shows settings and warriors side by side in wide windows ([e27a44c](https://github.com/yggdrion/valheim-tracker/commit/e27a44c05310d950ca5cbe737616ff6b7f650afa))

### 📝 Documentation

* README screenshots of the hall, map, road, achievements, a warrior page and Tracker Sync ([fed3ac2](https://github.com/yggdrion/valheim-tracker/commit/fed3ac2c7ac0eda8593e6cc04f73215562a47b19))
* restructure the README; Tracker Sync screenshot with server and token ([96c4c3b](https://github.com/yggdrion/valheim-tracker/commit/96c4c3bacb1b5b60ed0a0c50acf191a7cdbb7b51))

## 1.0.0 (2026-11-26)

### ✨ Features

* dashboard rework part 2: one slay row, game names, resting/empty states, phone layout ([27c83bb](https://github.com/yggdrion/valheim-tracker/commit/27c83bb842259efa01f9ccf301a1152e34d1f303))
* dashboard rework: saga stats, the road per biome, player pages, categories ([d4b39e4](https://github.com/yggdrion/valheim-tracker/commit/d4b39e446d11197cb223942393e2da9398d6bc5d))
* map page with biomes from the world seed, explored area, altars, traders, dungeons ([6e1b44d](https://github.com/yggdrion/valheim-tracker/commit/6e1b44d099e3becebc321502b19a8fefa4f3c722)), closes [#map](https://github.com/yggdrion/valheim-tracker/issues/map)
* raw .fch upload, parse on the server, bash client; drop the Go parser ([0851f43](https://github.com/yggdrion/valheim-tracker/commit/0851f43d215dc47c77b0d1230f708f17f38c6411))
* Tracker Sync desktop client (Tauri) and its build workflow ([b76d52d](https://github.com/yggdrion/valheim-tracker/commit/b76d52d440bbf8c43b73bb07d4ce301f8ca30f40))
* valheim achievement tracker (client, receiver, dashboard, docker) ([f8b3ffb](https://github.com/yggdrion/valheim-tracker/commit/f8b3ffb1f023b5f306b60c86b5cc098af0d10077))
* Windows client fchsync.ps1 and fchsync.bat ([0de8878](https://github.com/yggdrion/valheim-tracker/commit/0de8878eeafaad06a3909e1b228ba918035d1fc7))

### 📝 Documentation

* gui client replaces the ps1 script ([0c44bfb](https://github.com/yggdrion/valheim-tracker/commit/0c44bfb02b62b8c8d07692c8c948ffce70090c49))
* gui client selects characters ([73917fc](https://github.com/yggdrion/valheim-tracker/commit/73917fc0277660c3387bb2190389a7e5b733eeb9))
* plan raw-upload clients and drop the Go parser ([2c2d9ab](https://github.com/yggdrion/valheim-tracker/commit/2c2d9abb7cb0ad984cae98143db1dcd5786110ba))
* todo ghcr image, gui client; tls via separate proxy ([ffc6bf0](https://github.com/yggdrion/valheim-tracker/commit/ffc6bf082ca0e50738825b6f57916e5bf2afa995))
* todo: ghcr image done ([6e83a35](https://github.com/yggdrion/valheim-tracker/commit/6e83a35e8c045aa34ceee08ac8b4664b48acc1a3))

### 👷 CI

* automatic releases with semantic-release on every push to main ([7eb91bb](https://github.com/yggdrion/valheim-tracker/commit/7eb91bb5704a04d8aeee73955495bb5a0e10c790))
* one workflow builds the image and the app from the same trigger ([0a2f433](https://github.com/yggdrion/valheim-tracker/commit/0a2f433c5e650a3e967e7ce22b32b580f765ee2d))
* publish the tracker image to ghcr.io; compose uses it ([3e4a66f](https://github.com/yggdrion/valheim-tracker/commit/3e4a66f4432cd43ba908b5e1e232cb88c25589a7))
* release job downloads only the app installers ([64e0875](https://github.com/yggdrion/valheim-tracker/commit/64e0875da8c5944d1ada6941acb7eba309238b8e))
