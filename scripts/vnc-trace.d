#!/usr/sbin/dtrace -s

/*
 * vnc-trace.d     Print propolis VNC activity.
 *
 * USAGE: ./vnc-trace.d -p propolis-pid
 */

#pragma D option quiet

propolis$target:::rfb_framebuffer_update /* (bytes, interval_ms) */
{
    if (arg0 > 4) {
        printf("[%Y] Framebuffer Update: %d bytes, %d ms\n", walltimestamp, arg0, arg1);
    }
}

dtrace:::END
{
}
