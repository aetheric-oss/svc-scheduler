## [Release 0.2.1-develop.0](https://github.com/Arrow-air/svc-scheduler/releases/tag/v0.2.1-develop.0)

### 🐛 Fixes

-  **grpc:** server and clients start ([`6efa909`](https://github.com/Arrow-air/svc-scheduler/commit/6efa909a1953796cb712a9f878de9f8cdd8b66ba))

### 🛠 Maintenance

-  **ci:** update changelog ([`0234732`](https://github.com/Arrow-air/svc-scheduler/commit/023473211a26c1ad9378c771cedeaf96251bba62))
-  **ci:** update package version ([`719ca0b`](https://github.com/Arrow-air/svc-scheduler/commit/719ca0bdf1fad038279dce4efae9eb51e447466c))
-  **ci:** update package version<br/><br/>[skip ci] ([`f00396f`](https://github.com/Arrow-air/svc-scheduler/commit/f00396ffc79b8466b337ea36b75c325704db980c))
-  **ci:** .make/rust.mk - provisioned by terraform ([`ee8b85c`](https://github.com/Arrow-air/svc-scheduler/commit/ee8b85c26efe6e686aa8ca2ae9dcd931dd57f1a2))
-  **ci:** dockerfile - provisioned by terraform ([`d6e226b`](https://github.com/Arrow-air/svc-scheduler/commit/d6e226b670bee08a79675917b77e5e41f4665ccd))
-  **ci:** .github/workflows/release.yml - provisioned by terraform ([`e436d57`](https://github.com/Arrow-air/svc-scheduler/commit/e436d572e7561fd8996e961d5e561028adf79be6))

## [Release 0.2.0-develop.2](https://github.com/Arrow-air/svc-scheduler/releases/tag/v0.2.0-develop.2)

### 🛠 Maintenance

-  **ci:** update changelog ([`7e1d768`](https://github.com/Arrow-air/svc-scheduler/commit/7e1d7683b10479673e1652c8af3a4f54a744ed1d))
-  **ci:** update package version ([`f13049a`](https://github.com/Arrow-air/svc-scheduler/commit/f13049ae83fa18f713b95addf5c9d8555016fc5f))
-  **ci:** update package version<br/><br/>[skip ci] ([`c3cad4f`](https://github.com/Arrow-air/svc-scheduler/commit/c3cad4f9f73dd516db82eef5524b0c3b79e354b6))
- refactor code to lib-router, change fields on QueryFlightPlan ([`c13571d`](https://github.com/Arrow-air/svc-scheduler/commit/c13571d11c16112cb7dc3fd57afc489537322db7))

### 📚 Documentation

- update ICD document ([`ffe243c`](https://github.com/Arrow-air/svc-scheduler/commit/ffe243c365c3e2f90f9d4083b04af116d7fcf833))

## [Release 0.2.0-develop.1](https://github.com/Arrow-air/svc-scheduler/releases/tag/v0.2.0-develop.1)

### 🛠 Maintenance

-  **ci:** update changelog ([`44bf327`](https://github.com/Arrow-air/svc-scheduler/commit/44bf327bc02f5a05309e10fe9c43cf5efa083723))
-  **ci:** update package version ([`6c65681`](https://github.com/Arrow-air/svc-scheduler/commit/6c656810268afd35e0c3303af196e85f1c463462))
-  **ci:** update package version<br/><br/>[skip ci] ([`4361167`](https://github.com/Arrow-air/svc-scheduler/commit/43611676a4a4b372daf385492335719beda2031a))
- update code to work with new version of svc-storage and test data ([`83705c9`](https://github.com/Arrow-air/svc-scheduler/commit/83705c91a74c6615a4740f6761e9fb3597db3a58))

## [Release 0.2.0-develop.0](https://github.com/Arrow-air/svc-scheduler/releases/tag/v0.2.0-develop.0)

### ✨ Features

- add calendar utils to parse RFC 5545 rrules and check schedule availability ([`9f0d1f7`](https://github.com/Arrow-air/svc-scheduler/commit/9f0d1f72fb2db70d04563a5ba5d79b42b20f64a6))
- implement query_flight algo (without storage calls) ([`e85256a`](https://github.com/Arrow-air/svc-scheduler/commit/e85256a032403dc0ab5452f95352c3b1365bf2ed))
- implement query_flight with either departure or arrival time ([`251e242`](https://github.com/Arrow-air/svc-scheduler/commit/251e2424173aa87f49f4188a003b479348affc03))

### 🛠 Maintenance

-  **ci:** .editorconfig - provisioned by terraform ([`5e3e890`](https://github.com/Arrow-air/svc-scheduler/commit/5e3e8909ca9019d1a212b7c8d3daf807a48066a2))
-  **ci:** .make/rust.mk - provisioned by terraform ([`c32e540`](https://github.com/Arrow-air/svc-scheduler/commit/c32e540748be0a314e653938f685da438d474ad9))
-  **ci:** .cargo-husky/hooks/pre-commit - provisioned by terraform ([`d0e55fc`](https://github.com/Arrow-air/svc-scheduler/commit/d0e55fc4134d601e5f4173933fa401fa429311df))
-  **ci:** dockerfile - provisioned by terraform ([`9051f2d`](https://github.com/Arrow-air/svc-scheduler/commit/9051f2de6a1d5717d773702f4d19759154924d11))
-  **ci:** .gitignore - provisioned by terraform ([`07d7066`](https://github.com/Arrow-air/svc-scheduler/commit/07d70665682a12cb3e812471b7cc948c4a4fb9ce))
-  **ci:** .cargo-husky/hooks/readme.md - provisioned by terraform ([`185a500`](https://github.com/Arrow-air/svc-scheduler/commit/185a500026df443663c3a5ad41cdbb8c8805b84b))
-  **ci:** .github/workflows/sanity_checks.yml - provisioned by terraform ([`9d367cc`](https://github.com/Arrow-air/svc-scheduler/commit/9d367ccb14e853a8652e4eb36ca07466ab2c9201))
-  **ci:** .github/codeowners - provisioned by terraform ([`5a7fa64`](https://github.com/Arrow-air/svc-scheduler/commit/5a7fa641060877deca5e635bfc9ddd65cbcbee33))
-  **ci:** contributing.md - provisioned by terraform ([`340eef4`](https://github.com/Arrow-air/svc-scheduler/commit/340eef4a0b8d73b4996ea71ca880c286e500d059))
-  **ci:** .make/editorconfig.mk - provisioned by terraform ([`f890ccc`](https://github.com/Arrow-air/svc-scheduler/commit/f890ccc2649caa8b1817296d3f8c01d6d2e66e24))
-  **ci:** .make/toml.mk - provisioned by terraform ([`c378697`](https://github.com/Arrow-air/svc-scheduler/commit/c378697f5d9f1968cce37ff00177add220b615ec))
-  **ci:** .make/docker.mk - provisioned by terraform ([`deaffb7`](https://github.com/Arrow-air/svc-scheduler/commit/deaffb707f96d272830cc2eb953ea84b9f86e456))
-  **ci:** .github/workflows/python_ci.yml - provisioned by terraform ([`b14d0ad`](https://github.com/Arrow-air/svc-scheduler/commit/b14d0ad85cdd79c8d631c89e62e1cf88862688aa))
-  **ci:** .make/markdown.mk - provisioned by terraform ([`d96e4bf`](https://github.com/Arrow-air/svc-scheduler/commit/d96e4bf2d09bab2fcdb8c1ccf45e4bbb4a6701a3))
-  **ci:** .taplo.toml - provisioned by terraform ([`f346c2c`](https://github.com/Arrow-air/svc-scheduler/commit/f346c2cb9e7677a23ce87090d97f6947725c5b39))
-  **ci:** .make/base.mk - provisioned by terraform ([`64120da`](https://github.com/Arrow-air/svc-scheduler/commit/64120da58a0934b095a2118d6118e7b2af95d80b))
-  **ci:** .github/workflows/rust_ci.yml - provisioned by terraform ([`8e07c1d`](https://github.com/Arrow-air/svc-scheduler/commit/8e07c1d044d27adedd76f05b026fa3dec9cce097))
-  **ci:** makefile - provisioned by terraform ([`e9cf121`](https://github.com/Arrow-air/svc-scheduler/commit/e9cf121e9a22eb0648fa6d0e4c5ff80a784aea0a))
-  **ci:** .github/workflows/pr_rebase.yml - provisioned by terraform ([`3ad8eb1`](https://github.com/Arrow-air/svc-scheduler/commit/3ad8eb1930705e3c30e7c208eb6411f5e7c7eeae))
-  **ci:** .make/python.mk - provisioned by terraform ([`feb431c`](https://github.com/Arrow-air/svc-scheduler/commit/feb431cb8d3ac16ff77e8fec55c8020b890251a3))
-  **ci:** .cargo/config.toml - provisioned by terraform ([`71692dd`](https://github.com/Arrow-air/svc-scheduler/commit/71692ddcf710ad32e95e14e84707ea9f1adab211))
-  **ci:** .make/cspell.mk - provisioned by terraform ([`2ddb74c`](https://github.com/Arrow-air/svc-scheduler/commit/2ddb74c34c201385b4a79c6b66c0f09f3756d63c))
-  **ci:** .cargo-husky/hooks/pre-push - provisioned by terraform ([`17095bf`](https://github.com/Arrow-air/svc-scheduler/commit/17095bfe7870e8459e840b7d30046d65c04d1d76))
-  **ci:** .cspell.config.yaml - provisioned by terraform ([`aa3d2ec`](https://github.com/Arrow-air/svc-scheduler/commit/aa3d2ec207511ef5d030b739dfd5b30d235acc16))
-  **ci:** .make/rust.mk - provisioned by terraform ([`49d54f0`](https://github.com/Arrow-air/svc-scheduler/commit/49d54f032af404008e2e860aae26189004fa763c))
-  **ci:** docker-compose.yml - provisioned by terraform ([`4f92a03`](https://github.com/Arrow-air/svc-scheduler/commit/4f92a03392cc72c6aa381f27754209f085b35eef))
-  **ci:** dockerfile - provisioned by terraform ([`ea20a32`](https://github.com/Arrow-air/svc-scheduler/commit/ea20a323133a9677eda9db90b121752dcc551508))
-  **ci:** .github/workflows/rust_ci.yml - provisioned by terraform ([`1a837c1`](https://github.com/Arrow-air/svc-scheduler/commit/1a837c14e6dd789b31ee9d36a8733d5c96719b75))
-  **ci:** .github/workflows/sanity_checks.yml - provisioned by terraform ([`24fca5b`](https://github.com/Arrow-air/svc-scheduler/commit/24fca5b02eb41d82ad59ad1792f35fc0ec4c4f03))
-  **ci:** makefile - provisioned by terraform ([`6312330`](https://github.com/Arrow-air/svc-scheduler/commit/63123308b95dfd8146042cfcfc4284a0337beed5))
-  **ci:** .make/commitlint.mk - provisioned by terraform ([`858b47e`](https://github.com/Arrow-air/svc-scheduler/commit/858b47e27d6663bd379be17c008b5bbe0542011a))
-  **ci:** .commitlintrc.yml - provisioned by terraform ([`c3cda5d`](https://github.com/Arrow-air/svc-scheduler/commit/c3cda5d92956c4d957a7a7e0879ed90e2e73a404))
-  **ci:** .make/base.mk - provisioned by terraform ([`80c052e`](https://github.com/Arrow-air/svc-scheduler/commit/80c052ed2f8c3e9b441603632c0d2688fb08c3f9))
-  **ci:** .cargo-husky/hooks/commit-msg - provisioned by terraform ([`2f9c1b5`](https://github.com/Arrow-air/svc-scheduler/commit/2f9c1b5df39162f552ba2aa478051a23363b683b))
-  **ci:** dockerfile - provisioned by terraform ([`7f0d7da`](https://github.com/Arrow-air/svc-scheduler/commit/7f0d7da5bced7150f364c535182e0e05ed37ea14))
-  **ci:** .make/toml.mk - provisioned by terraform ([`84eaeae`](https://github.com/Arrow-air/svc-scheduler/commit/84eaeae8a126d045689c38a78f71e3cd62c64d91))
-  **ci:** .make/docker.mk - provisioned by terraform ([`a4b8b97`](https://github.com/Arrow-air/svc-scheduler/commit/a4b8b97e1e33fa693f5559b62f22615651944c0b))
-  **ci:** docker-compose.yml - provisioned by terraform ([`25f37b7`](https://github.com/Arrow-air/svc-scheduler/commit/25f37b76fdbff5404c6050c911aaa1fe9cb53cb5))
-  **ci:** .taplo.toml - provisioned by terraform ([`181eed3`](https://github.com/Arrow-air/svc-scheduler/commit/181eed37d4622456a12e7a9e841a177d7108ca75))
-  **ci:** makefile - provisioned by terraform ([`872afe6`](https://github.com/Arrow-air/svc-scheduler/commit/872afe623b5392c8f4b03a55eb80f42afbc7bdf7))
-  **ci:** .make/rust.mk - provisioned by terraform ([`7a8f9cb`](https://github.com/Arrow-air/svc-scheduler/commit/7a8f9cb030a17b1bb23a0b46256c474997b66247))
-  **ci:** .make/base.mk - provisioned by terraform ([`f741d48`](https://github.com/Arrow-air/svc-scheduler/commit/f741d48af627dab0cdcb1de8de4d34f7d98f8190))
-  **ci:** makefile - provisioned by terraform ([`0d86982`](https://github.com/Arrow-air/svc-scheduler/commit/0d869827ad4eac3453f03761cd005f432593fc15))
-  **ci:** .github/workflows/release.yml - provisioned by terraform ([`bc3482a`](https://github.com/Arrow-air/svc-scheduler/commit/bc3482a5c96f47c3cf29b3d3acf6ec8a00d09a8f))
-  **ci:** .make/rust.mk - provisioned by terraform ([`55c0525`](https://github.com/Arrow-air/svc-scheduler/commit/55c0525bb7419264d23c34019114d702cbd4fb31))
-  **ci:** dockerfile - provisioned by terraform ([`1308db8`](https://github.com/Arrow-air/svc-scheduler/commit/1308db88c6324a6e25be45c303e6afe62d645b65))
-  **ci:** docker-compose.yml - provisioned by terraform ([`a8cfee9`](https://github.com/Arrow-air/svc-scheduler/commit/a8cfee9e72997240591586a4fcc39db4eb5afb53))
-  **ci:** add CHANGELOG.md and fix docker build ([`a6179da`](https://github.com/Arrow-air/svc-scheduler/commit/a6179dac227a5b6c943b869893ebbbf345960674))

