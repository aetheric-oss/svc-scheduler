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


