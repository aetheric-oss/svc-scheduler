## [Release 0.5.0](https://github.com/aetheric-oss/svc-scheduler/releases/tag/v0.5.0)

### ✨ Features

- priority queues ([`a7e9c62`](https://github.com/aetheric-oss/svc-scheduler/commit/a7e9c62aeee7565f265a6e7e054ed0dcba446444))
- add expiry argument to create itinerary ([`90d5cbe`](https://github.com/aetheric-oss/svc-scheduler/commit/90d5cbeedf4e944a83a86144791e098d48daf08d))
- add user ID to request to create or cancel itinerary ([`8d9d711`](https://github.com/aetheric-oss/svc-scheduler/commit/8d9d7113b223a6285d5d3a56309fa4fc9d0aca2c))
- add flight path to svc-gis ([`d01a9c3`](https://github.com/aetheric-oss/svc-scheduler/commit/d01a9c38b93dac9589716f679938ceda9002e9fb))
- update deprecated chrono Duration calls ([`a9b2686`](https://github.com/aetheric-oss/svc-scheduler/commit/a9b26866403cf82c1ebe4c4fa3659a433ff65a47))
- added session id to flight plan ([`3247ab5`](https://github.com/aetheric-oss/svc-scheduler/commit/3247ab52714e0bbecbc5a58a9cd9d4f258e6e589))
- order by timeslot instead of aircraft ([`f926b47`](https://github.com/aetheric-oss/svc-scheduler/commit/f926b47ae57ee56dcea57bd08be12323ca3dbc2c))
- added altitude information to flight plans ([`7a14add`](https://github.com/aetheric-oss/svc-scheduler/commit/7a14addbb8ca7865f134dcd3bb8ccee05d6103a2))
- update lib-common references ([`73d4fbc`](https://github.com/aetheric-oss/svc-scheduler/commit/73d4fbc2baf403952fabe20d5998bc1a25182cd0))
- update unit tests ([`2064d61`](https://github.com/aetheric-oss/svc-scheduler/commit/2064d6189189ebf68bf29ad2f46841c239bc86e1))

### 🐛 Fixes

- remove unused files ([`64d8c57`](https://github.com/aetheric-oss/svc-scheduler/commit/64d8c57c1c6b49b90c8305f1943a9775ef006c10))
- fix task success bug, add result field to task response ([`44d1dd5`](https://github.com/aetheric-oss/svc-scheduler/commit/44d1dd5f57bcb3255f6bd3198ff898c8cdd4f39f))
- prevent origin timeslots prior to current time ([`8177299`](https://github.com/aetheric-oss/svc-scheduler/commit/81772996ba20afa9ab0f58d9246a288c5a349547))
- clippy errors ([`f1c8efb`](https://github.com/aetheric-oss/svc-scheduler/commit/f1c8efb30e5a593c4146b7dd20e19693f4d15c03))
- eliminate default filter call ([`73f379e`](https://github.com/aetheric-oss/svc-scheduler/commit/73f379e7e9f7d42930ab58390005988dad586173))
- add error log to config unwrap fail ([`7659f59`](https://github.com/aetheric-oss/svc-scheduler/commit/7659f595f2ae907dca99b2a394c2db6e230040c1))

### 🛠 Maintenance

- terraform provisioned file changes ([`27b2ab1`](https://github.com/aetheric-oss/svc-scheduler/commit/27b2ab12c3573389bada96fd31e3559eb2e4e2ef))
- upgrade cargo dependencies ([`7a72669`](https://github.com/aetheric-oss/svc-scheduler/commit/7a72669c014582fbb54f604a4bc196ef2cbe7fad))
- update spellcheck ([`895cf7f`](https://github.com/aetheric-oss/svc-scheduler/commit/895cf7f3cd2ca23614c9373a50198e26f77f2c7a))
- tofu provisioned file changes ([`7a75e4d`](https://github.com/aetheric-oss/svc-scheduler/commit/7a75e4d041b28459db46cba78d80a2a36c2e812a))
- update dependencies ([`87b0369`](https://github.com/aetheric-oss/svc-scheduler/commit/87b036939ab17e6b6d37f9d81a346494ec245f7a))
- final r4 updates ([`4e63c13`](https://github.com/aetheric-oss/svc-scheduler/commit/4e63c1308e29b30191f57f1afe860793f9d0ec4f))
- reviewer comments 1 ([`93970d3`](https://github.com/aetheric-oss/svc-scheduler/commit/93970d3b02af41cf876545dacdd41137586a4742))
- reviewer comments 2 ([`7e4a76a`](https://github.com/aetheric-oss/svc-scheduler/commit/7e4a76a936020898737fdd37d68a2768f6ddde4c))

## [Release 0.4.0](https://github.com/Arrow-air/svc-scheduler/releases/tag/v0.4.0)

### ✨ Features

- add prelude module ([`21f68f8`](https://github.com/Arrow-air/svc-scheduler/commit/21f68f8178e2ccec093b0f85f295da7bf513d053))
- use svc-gis for routing ([`f9d5298`](https://github.com/Arrow-air/svc-scheduler/commit/f9d52987a4780c24630619939a09e0517604dee0))

### 🐛 Fixes

- re-export required modules ([`2d23258`](https://github.com/Arrow-air/svc-scheduler/commit/2d23258ee847ae16de05841e245468d791929d77))
- schedule bug ([`f7bf638`](https://github.com/Arrow-air/svc-scheduler/commit/f7bf638c5a96272f4b741b2868124c561ff2cb41))
- unit test confirm cancel datetime bug ([`40bf0b6`](https://github.com/Arrow-air/svc-scheduler/commit/40bf0b604f545030d9c2efbfddab006028ba5a13))

### 🔥 Refactorings

-  **server:** use lib-common traits and add mock features ([`12c5337`](https://github.com/Arrow-air/svc-scheduler/commit/12c5337f13fb58bdb1753b1568a7b2bff71943f3))
-  **client:** use lib-common traits and add mock features ([`998b6a7`](https://github.com/Arrow-air/svc-scheduler/commit/998b6a7c669a70155474de2c5f39a7eaa9882088))
- vertipad timeslots and best path ([`86d83ed`](https://github.com/Arrow-air/svc-scheduler/commit/86d83ed48990b87d7dfb12e1162af915a2853e64))

### 🛠 Maintenance

- terraform provisioned file changes ([`20040ef`](https://github.com/Arrow-air/svc-scheduler/commit/20040ef4563a0f492939e05d4c2fc3b8c714cdbc))
- use latest svc-storage-client-grpc with prelude ([`81b10e1`](https://github.com/Arrow-air/svc-scheduler/commit/81b10e158b95cdcb75bf7a69cd105c95ee67d1af))
- reviewer comments ([`4c436bd`](https://github.com/Arrow-air/svc-scheduler/commit/4c436bdbab6090e9073c4f5923590812a6cbf200))
- add unit test for FlightQuery try_from ([`40506a2`](https://github.com/Arrow-air/svc-scheduler/commit/40506a2ed4fdff385e40bf47b84c90984cf4e4e9))
- add unit tests ([`103b258`](https://github.com/Arrow-air/svc-scheduler/commit/103b258332ab6034c5bb186c6e5c81d90618caa6))
- r3 final cleanup ([`3717b66`](https://github.com/Arrow-air/svc-scheduler/commit/3717b6669a69e8033478064946167c6ee6cf2966))
- reviewer comments ([`8622c28`](https://github.com/Arrow-air/svc-scheduler/commit/8622c28b00d3a7e40a9a9415fb9b5f0bc2b592b4))

### 📚 Documentation

- fix headings, icons, and banners ([`ba1073c`](https://github.com/Arrow-air/svc-scheduler/commit/ba1073c546b3e5dd0eb810f6df9c8033d6c8ec85))

## [Release 0.3.0](https://github.com/Arrow-air/svc-scheduler/releases/tag/v0.3.0)

### ✨ Features

- new interface for query_flight ([`a40ddbe`](https://github.com/Arrow-air/svc-scheduler/commit/a40ddbee3cb68e1ba1912230b30e3cdf8876338b))
- time range implementation for query flight ([`2062046`](https://github.com/Arrow-air/svc-scheduler/commit/20620467c76711afb4197a00694a11b01c9cbe4f))
- call compliance service when confirming flight ([`6e17c32`](https://github.com/Arrow-air/svc-scheduler/commit/6e17c3212a0a94359cc6b26cc9c49c745fd41ee7))
- call compliance service when confirming flight ([`340415c`](https://github.com/Arrow-air/svc-scheduler/commit/340415c569d71ed4fee3e3ef5dd461dadb766b62))
- implement new scenarios with router and add new test ([`54c231f`](https://github.com/Arrow-air/svc-scheduler/commit/54c231f0953fd608f55109c8e365ead7ee004398))
- implement scenario 5 and test ([`46d2738`](https://github.com/Arrow-air/svc-scheduler/commit/46d273843e4024f8fc72f1a383457471b52fb721))
- implement scenario 6 and test ([`5ae4966`](https://github.com/Arrow-air/svc-scheduler/commit/5ae4966ddcb36f61cb744874ab77ad07cb647db2))
- add itineraries ([`03f4bfa`](https://github.com/Arrow-air/svc-scheduler/commit/03f4bfac051bc79b4c75a23765f23a3da707a91f))
- loop router initialization ([`2dd1153`](https://github.com/Arrow-air/svc-scheduler/commit/2dd115391178dc2d2844511bf8c9bab14efcf1eb))
- add conops ([`6803125`](https://github.com/Arrow-air/svc-scheduler/commit/68031259964d458f8968ffa33dff9d9a96799fb0))
- combine router lib ([`2faf70b`](https://github.com/Arrow-air/svc-scheduler/commit/2faf70b7c2d24dfc051cc3528fc4e41e69a78521))

### 🐛 Fixes

- fix queries with deleted flag ([`69782ac`](https://github.com/Arrow-air/svc-scheduler/commit/69782ac33bf4ed8d2a8fbc6bedf37a54377922c1))
- remove redundant doc comments ([`40bc03f`](https://github.com/Arrow-air/svc-scheduler/commit/40bc03fd1a12f4deba6ccb97bc40a492435ed49a))

### 🔥 Refactorings

- organize router module ([`8449def`](https://github.com/Arrow-air/svc-scheduler/commit/8449defbbed19cb785f7b3cd6ec785ed62c84dda))

### 🛠 Maintenance

- terraform provisioned file changes ([`712e75e`](https://github.com/Arrow-air/svc-scheduler/commit/712e75ebf9efd3c30dfd1f9cb6b0623ae5256d9c))
- refactor code to be testable ([`a3e92f2`](https://github.com/Arrow-air/svc-scheduler/commit/a3e92f2d120ba6cba715e96dee2af26505627914))
- update release files ([`4ee00ab`](https://github.com/Arrow-air/svc-scheduler/commit/4ee00abe4085f0b1d11b0cc091567c33601b9e0c))
- update svc-storage version with advanced filters ([`dbe49a7`](https://github.com/Arrow-air/svc-scheduler/commit/dbe49a76a6f05214eff278fabf0bef6f7be8e82e))
- refactor test data to return exp results with deadhead flights ([`507ace6`](https://github.com/Arrow-air/svc-scheduler/commit/507ace6040dfceb117a881eb3dcc5b8e103dcde4))
- add 2 tests for deadhead flights (one for parked and one for in-flight scenario) ([`d27261e`](https://github.com/Arrow-air/svc-scheduler/commit/d27261e3a6d1be1e4d6ae1a5f78812dd2f8144e1))
- update dependency ([`6606b79`](https://github.com/Arrow-air/svc-scheduler/commit/6606b7973b482eb37293af93a1f17c22c21a4435))
- add 1 test for aircraft re-routing when capacity not met ([`2f2f28c`](https://github.com/Arrow-air/svc-scheduler/commit/2f2f28c872d20a351a0c9713e86fe98dc41b2cad))
- update README ([`5104464`](https://github.com/Arrow-air/svc-scheduler/commit/51044640ae1dfd382b1bb08d0381539a22b6d617))
- module refactor ([`0971b79`](https://github.com/Arrow-air/svc-scheduler/commit/0971b790dee345d69635bce17032dc95620554ab))
- review cleanup ([`ba6df85`](https://github.com/Arrow-air/svc-scheduler/commit/ba6df8542df9d318add8c11f0d399a05bf227919))
- remove references to lib-router ([`348494a`](https://github.com/Arrow-air/svc-scheduler/commit/348494a92e45c0ac80d85a0e25868aef4b4f5888))
- address reviewer comments ([`cd81b97`](https://github.com/Arrow-air/svc-scheduler/commit/cd81b978e79180ca872b794a32e7fb03f7ff3663))

### 📚 Documentation

-  **readme:** add license notice and additional info (#24) ([`63001ba`](https://github.com/Arrow-air/svc-scheduler/commit/63001ba7134bf77490c72aeaa5c455e139417e40))

## [Release 0.2.0](https://github.com/Arrow-air/svc-scheduler/releases/tag/v0.2.0)

### ✨ Features

- initialize grpc server and client ([`c23cc5c`](https://github.com/Arrow-air/svc-scheduler/commit/c23cc5ced93a28cc10244595f364a8a74cfb15ca))
- add calendar utils to parse RFC 5545 rrules and check schedule availability ([`39ce1cc`](https://github.com/Arrow-air/svc-scheduler/commit/39ce1ccdc4b72e617612f3a64fcc33dc5f9b0fb6))
- implement query_flight algo (without storage calls) ([`44995b9`](https://github.com/Arrow-air/svc-scheduler/commit/44995b91007a9326fdfc9085403c712e27c6f335))
- implement query_flight with either departure or arrival time ([`46a3eed`](https://github.com/Arrow-air/svc-scheduler/commit/46a3eed2ac642670fae78641571dedbcb99eeb33))

### 🐛 Fixes

-  **grpc:** server and clients start ([`9220b35`](https://github.com/Arrow-air/svc-scheduler/commit/9220b3548fb7c8682633f7d15d7d3af0e84115f6))
-  **cargo:** add vendored-openssl feature ([`95b526d`](https://github.com/Arrow-air/svc-scheduler/commit/95b526db43357dc3884946af2a51d2945d27dbbc))
- cancel flight now will cancel draft and confirmed flight plans; error handling implemented ([`6fd4d78`](https://github.com/Arrow-air/svc-scheduler/commit/6fd4d7810ef46f0de0e3594c470f80b2f7496516))
- rename example ([`ebd3cf6`](https://github.com/Arrow-air/svc-scheduler/commit/ebd3cf67e56fa5e0d94a57fffc90ca4a45335eb9))
- use fp_id for cancel requests ([`48113bb`](https://github.com/Arrow-air/svc-scheduler/commit/48113bb6706b1fef2b8de5ea222e6de371dfba35))
- r1 review fixes ([`ac28ca1`](https://github.com/Arrow-air/svc-scheduler/commit/ac28ca197bdbb18f583659144a79e5940ef888c8))
- empty changelog ([`b209ecf`](https://github.com/Arrow-air/svc-scheduler/commit/b209ecf524f26f863b58775d324a28946277fc48))

### 🛠 Maintenance

-  **init:** initial repository setup ([`b89963b`](https://github.com/Arrow-air/svc-scheduler/commit/b89963b838f66d6e13422d8884efd4660e489bbf))
-  **ci:** provisioned by terraform ([`bfe85c5`](https://github.com/Arrow-air/svc-scheduler/commit/bfe85c5ed82d8732e45486a1616902f38a737359))
-  **ci:** provisioned by terraform ([`17a66a3`](https://github.com/Arrow-air/svc-scheduler/commit/17a66a32b49ff2a03c91577eb6b833c8bd76054a))
- update svc-storage ([`72bb79f`](https://github.com/Arrow-air/svc-scheduler/commit/72bb79f64be955a133aba66690fa32eab9fe2436))
- refactor code to lib-router, change fields on QueryFlightPlan ([`68f8c2c`](https://github.com/Arrow-air/svc-scheduler/commit/68f8c2c59dcb6e52916c1537a2bfd764cf47a3e0))
-  **grpc:** move grpc client to client-grpc folder ([`4866668`](https://github.com/Arrow-air/svc-scheduler/commit/4866668b39bf3f7f60b693162c3483eafed1bf0c))
-  **ci:** .make/docker.mk - provisioned by terraform ([`6291913`](https://github.com/Arrow-air/svc-scheduler/commit/6291913171919fd9f3c5ba88af3744e36cfd7dab))
-  **logging:** add logging capability and messages (#15) ([`aa6159d`](https://github.com/Arrow-air/svc-scheduler/commit/aa6159d20fe3e46f751bf93090635fa96b3408b8))
-  **logging:** change logger to log4rs ([`6b402e6`](https://github.com/Arrow-air/svc-scheduler/commit/6b402e667b7ddcda0dc555f23c343c10149159b4))

### 📚 Documentation

- update ICD document ([`084aeb6`](https://github.com/Arrow-air/svc-scheduler/commit/084aeb64da97fd00b4ed093eb69d381570b1777a))
- update ICD document ([`9ab61f6`](https://github.com/Arrow-air/svc-scheduler/commit/9ab61f60c0a93f8f6a3768d21c927dff1024e370))
-  **sdd:** add sdd document ([`f899a89`](https://github.com/Arrow-air/svc-scheduler/commit/f899a89d820b4e5c2a817a4c5929cca98ed6a403))
- update readme file ([`a0fb10c`](https://github.com/Arrow-air/svc-scheduler/commit/a0fb10c3ae5e9e94ba798bc3dfc6acab1740d5bc))
