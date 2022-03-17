Top of `::stack`:
```
> ::stack
_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+0xb()
_ZN15propolis_server3vnc6server13VncConnection7process17h02cddf854b238788E+0xf35()
_ZN15propolis_server3vnc6server9VncServer5start28_$u7b$$u7b$closure$u7d$$u7d$28_$u7b$$u7b$closure$u7d$$u7d$17h35db5cb5736f03e5E+0x19e()
...
```

Log output:
```
jordan@atrium ~/propolis $ bunyan propolis-log.json 
[2022-03-17T17:22:58.590307826Z]  INFO: propolis-server/vnc-server/1389 on atrium: vnc-server: starting...
[2022-03-17T17:22:58.590804702Z]  INFO: propolis-server/1389 on atrium: Starting server...
[2022-03-17T17:23:13.503743635Z]  INFO: propolis-server/vnc-server/1389 on atrium: vnc-server: got connection
[2022-03-17T17:23:13.504157317Z]  INFO: propolis-server/vnc-server/1389 on atrium: vnc-server: spawned
[2022-03-17T17:23:13.504528933Z]  INFO: propolis-server/vnc-server/1389 on atrium: BEGIN: ProtocolVersion Handshake
[2022-03-17T17:23:13.504891838Z]  INFO: propolis-server/vnc-server/1389 on atrium: tx: ProtocolVersion
[2022-03-17T17:23:13.505255704Z]  INFO: propolis-server/vnc-server/1389 on atrium: rx: ProtocolVersion
[2022-03-17T17:23:13.554317293Z]  INFO: propolis-server/vnc-server/1389 on atrium:
    END: ProtocolVersion Handshake
    
[2022-03-17T17:23:13.554644901Z]  INFO: propolis-server/vnc-server/1389 on atrium: BEGIN: Security Handshake
[2022-03-17T17:23:13.55495287Z]  INFO: propolis-server/vnc-server/1389 on atrium: tx: SecurityTypes
[2022-03-17T17:23:13.55525802Z]  INFO: propolis-server/vnc-server/1389 on atrium: rx: SecurityType
[2022-03-17T17:23:13.656881094Z]  INFO: propolis-server/vnc-server/1389 on atrium: tx: SecurityResult
[2022-03-17T17:23:13.657262938Z]  INFO: propolis-server/vnc-server/1389 on atrium:
    END: Security Handshake
    
[2022-03-17T17:23:13.65768404Z]  INFO: propolis-server/vnc-server/1389 on atrium: BEGIN: Initialization
[2022-03-17T17:23:13.657951212Z]  INFO: propolis-server/vnc-server/1389 on atrium: rx: ClientInit
{"msg":"tx: ServerInit","v":0,"name":"propolis-server","level":30,"time":"2022-03-17T17:23:13.708256148Z","
```


It looks like the segfault is for a bad address, `0xffffffffffcffc90` (instruction +0xb):
```
jordan@atrium ~/propolis $ dis -F '_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E' ./target/debug/propolis-server
disassembly for ./target/debug/propolis-server

_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E()
    _ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E:       55                 pushq  %rbp
    _ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+0x1:   48 89 e5           movq   %rsp,%rbp
    _ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+0x4:   48 81 ec c0 03 30  subq   $0x3003c0,%rsp
                                                                                                                                                 00 
    _ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+0xb:   48 89 bd 90 fc cf  movq   %rdi,0xffffffffffcffc90(%rbp)
                                                                                                                                                 ff 
    _ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+0x12:  48 89 b5 98 fc cf  movq   %rsi,0xffffffffffcffc98(%rbp)
                                                                                                                                                 ff 
    _ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+0x19:  48 89 bd 00 ff ff  movq   %rdi,0xffffffffffffff00(%rbp)
```

I think that's loading an argument (the first?) argument, presumably the TCP stream.

The assembly for the last `write_to` call, which is on a `SecurityResult`, looks more sane: 
```
jordan@atrium ~/propolis $ dis -F '_ZN96_$LT$propolis_server..vnc..rfb..SecurityResult$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h14f802c8d325b51bE' ./target/debug/propolis-server
disassembly for ./target/debug/propolis-server

_ZN96_$LT$propolis_server..vnc..rfb..SecurityResult$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h14f802c8d325b51bE()
    _ZN96_$LT$propolis_server..vnc..rfb..SecurityResult$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h14f802c8d325b51bE:       55                 pushq  %rbp
    _ZN96_$LT$propolis_server..vnc..rfb..SecurityResult$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h14f802c8d325b51bE+0x1:   48 89 e5           movq   %rsp,%rbp
    _ZN96_$LT$propolis_server..vnc..rfb..SecurityResult$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h14f802c8d325b51bE+0x4:   48 81 ec b0 00 00  subq   $0xb0,%rsp
                                                                                                                                              00 
    _ZN96_$LT$propolis_server..vnc..rfb..SecurityResult$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h14f802c8d325b51bE+0xb:   48 89 b5 58 ff ff  movq   %rsi,-0xa8(%rbp)
                                                                                                                                              ff 
    _ZN96_$LT$propolis_server..vnc..rfb..SecurityResult$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h14f802c8d325b51bE+0x12:  48 89 7d b0        movq   %rdi,-0x50(%rbp)
    _ZN96_$LT$propolis_server..vnc..rfb..SecurityResult$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h14f802c8d325b51bE+0x16:  48 89 75 b8        movq   %rsi,-0x48(%rbp)
    _ZN96_$LT$propolis_server..vnc..rfb..SecurityResult$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h14f802c8d325b51bE+0x1a:  0f b6 07           movzbl (%rdi),%eax
```

The start of the `read_from` call for `ClientInit`, that last thing that touche the stream, looks a little suspicious:
```
jordan@atrium ~/propolis $ dis -F '_ZN92_$LT$propolis_server..vnc..rfb..ClientInit$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17hee88a0b624d70498E' ./target/debug/propolis-server
disassembly for ./target/debug/propolis-server

_ZN92_$LT$propolis_server..vnc..rfb..ClientInit$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17hee88a0b624d70498E()
    _ZN92_$LT$propolis_server..vnc..rfb..ClientInit$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17hee88a0b624d70498E:       55                 pushq  %rbp
    _ZN92_$LT$propolis_server..vnc..rfb..ClientInit$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17hee88a0b624d70498E+0x1:   48 89 e5           movq   %rsp,%rbp
    _ZN92_$LT$propolis_server..vnc..rfb..ClientInit$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17hee88a0b624d70498E+0x4:   48 81 ec 50 01 00  subq   $0x150,%rsp
                                                                                                                                           00 
    _ZN92_$LT$propolis_server..vnc..rfb..ClientInit$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17hee88a0b624d70498E+0xb:   48 89 7d b8        movq   %rdi,-0x48(%rbp)
    _ZN92_$LT$propolis_server..vnc..rfb..ClientInit$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17hee88a0b624d70498E+0xf:   c6 85 ef fe ff ff  movb   $0x0,0xfffffffffffffeef(%rbp)
                                                                                                                                           00 
    _ZN92_$LT$propolis_server..vnc..rfb..ClientInit$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17hee88a0b624d70498E+0x16:  48 8d b5 ef fe ff  leaq   0xfffffffffffffeef(%rbp),%rsi
```

I don't see anything similarly suspicious in the previous `read_from` call, to `SecurityType`:
```
jordan@atrium ~/propolis $ dis -F '_ZN94_$LT$propolis_server..vnc..rfb..SecurityType$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17h69a8c3ade8b46e9cE' ./target/debug/propolis-server
disassembly for ./target/debug/propolis-server

_ZN94_$LT$propolis_server..vnc..rfb..SecurityType$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17h69a8c3ade8b46e9cE()
    _ZN94_$LT$propolis_server..vnc..rfb..SecurityType$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17h69a8c3ade8b46e9cE:       55                 pushq  %rbp
    _ZN94_$LT$propolis_server..vnc..rfb..SecurityType$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17h69a8c3ade8b46e9cE+0x1:   48 89 e5           movq   %rsp,%rbp
    _ZN94_$LT$propolis_server..vnc..rfb..SecurityType$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17h69a8c3ade8b46e9cE+0x4:   48 81 ec 20 01 00  subq   $0x120,%rsp
                                                                                                                                             00 
    _ZN94_$LT$propolis_server..vnc..rfb..SecurityType$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17h69a8c3ade8b46e9cE+0xb:   48 89 7d c8        movq   %rdi,-0x38(%rbp)
    _ZN94_$LT$propolis_server..vnc..rfb..SecurityType$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17h69a8c3ade8b46e9cE+0xf:   c6 85 0f ff ff ff  movb   $0x0,-0xf1(%rbp)
                                                                                                                                             00 
    _ZN94_$LT$propolis_server..vnc..rfb..SecurityType$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17h69a8c3ade8b46e9cE+0x16:  48 8d b5 0f ff ff  leaq   -0xf1(%rbp),%rsi
                                                                                                                                             ff 
    _ZN94_$LT$propolis_server..vnc..rfb..SecurityType$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17h69a8c3ade8b46e9cE+0x1d:  ba 01 00 00 00     movl   $0x1,%edx
    _ZN94_$LT$propolis_server..vnc..rfb..SecurityType$u20$as$u20$propolis_server..vnc..rfb..Message$GT$9read_from17h69a8c3ade8b46e9cE+0x22:  e8 f9 70 0a 00     call   +0xa70f9 <_ZN3std2io4Read10read_exact17h6c9106d96c6e445fE>
```
