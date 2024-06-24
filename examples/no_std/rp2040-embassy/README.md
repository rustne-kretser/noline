# Embassy async enabled USB CDC interface

This example code shows how to use the embassy async system to noline in a no_std environment.

It has only been tested on the RP2040, but should be simple to port to other platforms supported by embassy.

It has two async functions running to demonstrate that the two can co-exist.

- blink::blinking_led which flashes the status LED on a regular cadance
- noline_async::cli which reads and writes to the USB CDC serial interface
