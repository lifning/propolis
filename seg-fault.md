I recently saw my VNC server code, new code in propolis-server, hit a segfault and die, seemingly when writing to a TcpStream from inside a tokio task.

It died because of a bad address, `0xfffffc7fe16fd7b8`:
```
> ::status
debugging core file of propolis-server (64-bit) from atrium
initial argv: /home/jordan/propolis/target/debug/propolis-server run confs/example-server.con
threading model: native threads
status: process terminated by SIGSEGV (Segmentation Fault), addr=fffffc7fe16fd7b8
```

This was the top of the stack:
```
> ::stack
_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+0xb()
_ZN15propolis_server3vnc6server13VncConnection7process17h02cddf854b238788E+0xf35()
_ZN15propolis_server3vnc6server9VncServer5start28_$u7b$$u7b$closure$u7d$$u7d$28_$u7b$$u7b$closure$u7d$$u7d$17h35db5cb5736f03e5E+0x19e()
...
```

It died trying to write to a TcpStream -- specifically, trying to write a FramebufferUpdate packet. I took a look at the assembly of the function where it died to get a better sense of what it was doing (XXX: comment about dis with `$`; another alternative `<rip::dis`:


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

...

```

One thing that jumped out at me here is a bunch of suspicious looking addresses that start with many `f`s, similar to the address that caused the segmentation fault.
Then I looked more closely at the specific instruction where the program died:
```
    _ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+0xb:   48 89 bd 90 fc cf  movq   %rdi,0xffffffffffcffc90(%rbp)
```

So this instruction is saying (XXX: note about AT&T syntax vs intel): get the address out of the base pointer (`%rbp`), add `0xffffffffffcffc90` to it, get the value at the resulting address and store it in the `%rdi` register.

I took a look at the register state:
```
> ::regs
%rax = 0xfffffc7fe19fe2c0       %r8  = 0x0000000000000010
%rbx = 0x0000000004d6b4a8       %r9  = 0x0000000004d1a678
%rcx = 0x0000000000000001       %r10 = 0x0000000000000003
%rdx = 0x0000000000000001       %r11 = 0xfffffc7fee5e0cb0
%rsi = 0xfffffc7fe19fe488       %r12 = 0x000000000582d650
%rdi = 0xfffffc7fe19fe2c0       %r13 = 0x0000000000000000
                                %r14 = 0xfffffc7fee321000
                                %r15 = 0x000000000582e7d0

%cs = 0x0053    %fs = 0x0000    %gs = 0x0000
%ds = 0x004b    %es = 0x004b    %ss = 0x004b

%rip = 0x0000000002a4f6db _ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+0xb
%rbp = 0xfffffc7fe19fda20
%rsp = 0xfffffc7fe16fd770

%rflags = 0x00010282
  id=0 vip=0 vif=0 ac=0 vm=0 rf=1 nt=0 iopl=0x0
  status=<of,df,IF,tf,SF,zf,af,pf,cf>

%gsbase = 0x0000000000000000
%fsbase = 0xfffffc7fee527a40
%trapno = 0xe
   %err = 0x6
```

Then calculated the address as the instruction that caused the fault did:
```
> 0xffffffffffcffd98 + <rbp = K
                fffffc7fe16fd7b8
```

Sure enough, that's the garbage address. I took a look at the memory mappings of the program around that address:

```
> ::mappings
            BASE            LIMIT             SIZE NAME
          400000          4c36000          4836000 /home/jordan/propolis/target/debug/propolis-server
         4c45000          4d7f000           13a000 /home/jordan/propolis/target/debug/propolis-server
         571b000          585b000           140000 [ heap ]
fffffc7fd7bfe000 fffffc7fd7c00000             2000 [ unknown ]
fffffc7fde9fe000 fffffc7fdea00000             2000 [ unknown ]
fffffc7fdf5fe000 fffffc7fdf600000             2000 [ unknown ]
fffffc7fe05fe000 fffffc7fe0600000             2000 [ unknown ]
fffffc7fe09f9000 fffffc7fe0a00000             7000 [ unknown ]
fffffc7fe0dfc000 fffffc7fe0e00000             4000 [ unknown ]
fffffc7fe13fe000 fffffc7fe1400000             2000 [ unknown ]
fffffc7fe19fc000 fffffc7fe1a00000             4000 [ unknown ]
fffffc7fe1dfe000 fffffc7fe1e00000             2000 [ unknown ]
fffffc7fe21fe000 fffffc7fe2200000             2000 [ unknown ]
fffffc7fe27fe000 fffffc7fe2800000             2000 [ unknown ]
fffffc7fe2bfe000 fffffc7fe2c00000             2000 [ unknown ]
fffffc7fe2ffe000 fffffc7fe3000000             2000 [ unknown ]
fffffc7fe35fe000 fffffc7fe3600000             2000 [ unknown ]
fffffc7fe39fe000 fffffc7fe3a00000             2000 [ unknown ]
fffffc7fe3dfe000 fffffc7fe3e00000             2000 [ unknown ]
fffffc7fe41fe000 fffffc7fe4200000             2000 [ unknown ]
fffffc7fe45fe000 fffffc7fe4600000             2000 [ unknown ]
fffffc7fe464e000 fffffc7fe46a2000            54000 /lib/amd64/ld.so.1
fffffc7fe46b2000 fffffc7fe46b5000             3000 /lib/amd64/ld.so.1
fffffc7fe46b5000 fffffc7fe46b7000             2000 /lib/amd64/ld.so.1

...

Not surprisingly, `0xfffffc7fe16fd7b8` is not mapped. It looks like if it were, it would be in a region of anonymous memory, presumably for tokio's stack. At this point, I took a closer look at the assembly again to see where the stack was being setup:

```
> <rip::dis                           
_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E:  pushq  %rbp
_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+1:movq   %rsp,%rbp
_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+4:subq   $0x3002b0,%rsp
_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+0xb:      movq   %rdi,0xffffffffffcffd98(%rbp)
```

Instruction `+0x4` creates the stack, and it's grabbing quite a bit of memory: `0x3002b0`, which is about 3 MB:
```
> 0x3002b0=E
                3146416  
```

That seems very suspicious.
XXX: talk about repro'ing on mac 


This log output shows about how far it made it in to the protocol:
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
