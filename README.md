# lincstation-n2-leds-rusty
Rust based daemon for lincstation n2 leds

a lot to do here

it's a daemon, but can be run as a process one time to update all the leds by the logic.

we will have:
1. daemon mode, effectively a single run every 1s.
2. debug run (dumps debug output and is a read-only flow)
3. single run (runs once, updates led colors, updates the log, exits)

log will always show the some finite amount of state, maybe the last 3 runs or something. we don't care about long term data.

disk status service tries it's best to collect disk status in an isolated manner. additionally this feeds the debug and logging output.

it's core purpose is to produce consistent output that can be used to map out the various leds desired color states.

i.e.:
Power Switch LED -> On, white
Led 1, On, blue
Led 2, On, white
Led 3, On, white
Led 4, On, green
Led 5, On, blue
Led 6, On, red
Led 7, On, red
LED Bar, breathing, On, white


the high level idea is that the disk status service will periodically get the disk status and the daemon will "save that" in some way.

the led controller service will be given that to "drive" the correct led colors, brightness, etc

at the end of this when i am rewriting this, we will verify the "reusability" of this by looking at what would happen with a similar linux system but different led controller. i don't look to address it, just confirm that if a better "led driver" becomes available, i can plug it in if it's not linux native ezpz mode.
