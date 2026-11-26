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
