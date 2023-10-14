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
