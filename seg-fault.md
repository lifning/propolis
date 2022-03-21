I recently saw my VNC server code in propolis-server, crash due to a segfault.
This would happen after a client connected to it, and the server started to
send framebuffer data.

The server died because of a bad address, `0xfffffc7fe16fd7b8`:
```
> ::status
::status
debugging core file of propolis-server (64-bit) from atrium
initial argv: /home/jordan/propolis/target/debug/propolis-server run confs/example-server.con
threading model: native threads
status: process terminated by SIGSEGV (Segmentation Fault), addr=fffffc7fe16fd7b8
```

This log output shows about how far it made it in to the RFB protocol:
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

This was the top of the stack:
```
> ::stack
_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+0xb()
_ZN15propolis_server3vnc6server13VncConnection7process17h02cddf854b238788E+0xf35()
_ZN15propolis_server3vnc6server9VncServer5start28_$u7b$$u7b$closure$u7d$$u7d$28_$u7b$$u7b$closure$u7d$$u7d$17h35db5cb5736f03e5E+0x19e()
_ZN97_$LT$core..future..from_generator..GenFuture$LT$T$GT$$u20$as$u20$core..future..future..Future$GT$4poll17h76747e47c50d6ff3E+0x50()
_ZN5tokio7runtime4task4core18CoreStage$LT$T$GT$4poll28_$u7b$$u7b$closure$u7d$$u7d$17h47b7370aa914d74aE+0xde()
_ZN5tokio4loom3std11unsafe_cell19UnsafeCell$LT$T$GT$8with_mut17hc438ab5cef3086b9E+0x31()
...
```

This code is from a tokio task trying to a write FramebufferUpdate message to a
TcpStream between the server and the client.  I took a look at the assembly of
the function where it died to get a better sense of what it was doing:


```
> <rip::dis
_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E:  pushq  %rbp
_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+1:movq   %rsp,%rbp
_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+4:subq   $0x3002b0,%rsp
_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+0xb:      movq   %rdi,0xffffffffffcffd98(%rbp)
_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+0x12:     movq   %rsi,0xffffffffffcffda0(%rbp)
_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+0x19:     movq   %rdi,-0xd8(%rbp)
_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+0x20:     movq   %rsi,-0xd0(%rbp)
_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+0x27:     movb   $0x0,0xffffffffffcffddf(%rbp)
...
```

One thing that jumped out at me here is a bunch of suspicious looking addresses
that start with many `f`s, similar to the address that caused the segmentation
fault.  Then I looked more closely at the specific instruction where the
program died (`+0xb`):
```
_ZN99_$LT$propolis_server..vnc..rfb..FramebufferUpdate$u20$as$u20$propolis_server..vnc..rfb..Message$GT$8write_to17h31cde3ba3d358797E+0xb:      movq   %rdi,0xffffffffffcffd98(%rbp)
```

The first thing to remember is that mdb uses AT&T syntax (thus the order of
arguments for the `mov` is `src, dest`). So this instruction is saying: load the
value in rdi, and store it at the address calculated from the value of the
base pointer (rbp) plus `0xffffffffffcffd98`.

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

Sure enough, that's the garbage address that caused the SIGSEGV. I took a look
at the memory mappings of the program around that address:

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
```

...

Not surprisingly, `0xfffffc7fe16fd7b8` is not mapped. It looks like if it were,
it would be in a region of anonymous memory, presumably for tokio's stack. At
this point, I took a closer look at the assembly again to see where the stack
was being setup:

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

In parallel with looking at the propolis-server core file, I tried to reproduce
the SIGSEGV by making a minimal example using this code outside of propolis on
my macbook. I didn't see a segfault, but I did see a stack overflow:

```
thread 'tokio-runtime-worker' has overflowed its stack
fatal runtime error: stack overflow
zsh: abort      cargo run
```

It seemed likely that something was causing tokio to try to allocate a stack bigger than it should. At this point, I went to look to see if something in the code I wrote might be allocating something too big on the stack. And sure enough, I was allocating a pretty large array for some placeholder framebuffer data (with the intent of cleaning this code up later). Changing the line to use `vec!` or a `Box` fixed both the segfault on illumos and the stack overflow on my mac.
