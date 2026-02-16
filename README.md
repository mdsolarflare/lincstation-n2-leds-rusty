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


the physical case has leds like this (top to bottom is left to right on the machine)
LED1 -> SSD1
LED2 -> SSD2
LED3 -> NVME1
LED4 -> NVME2
LED5 -> NVME3
LED6 -> NVME4
LED7 -> <...> (a management port symbol?)


## LED Controller Hardware (Reverse Engineered)

The LED controller chip is an unknown I2C/SMBus device located at address **0x26** on the system's I2C bus. Communication happens via SMBus byte read/write operations. The device has two distinct subsystems: the LED bar (chassis status light) and 8 individual LED strips for drive indicators.

### LED Bar (Chassis Status Light)

The LED bar has three display modes and uses RGB color control via SMBus registers:

| Register  | Mode | Purpose                                        |
|-----------|------|------------------------------------------------|
| 0x90      | RW   | Mode control: 0=Solid, 1=Breath, 2=Loop        |
| 0x91      | RW   | Brightness: 0x00 (off) to 0xFF (brightest)     |
| 0x92-0x94 | RW   | Color RGB for Solid and Breath modes (R, G, B) |
| 0x95-0x97 | RW   | Loop color A RGB (R, G, B)                     |
| 0x98-0x9A | RW   | Loop color B RGB (R, G, B)                     |

**Key findings:**
- Solid and Breath modes share the same color registers (0x92-0x94)
- Loop mode cycles between two colors (A and B)
- Color depth is 8-bit per channel (0-255)
- All three color sets can be safely written, only the active mode's colors are displayed

### LED Strips (8 Drive Indicators)

The 8 LED strips (POWER/MGMT/SATA1/SATA2/NVME1-4) are controlled via SMBus registers. Each strip has **two independent channels: white and red**. When both are on simultaneously, the LED appears orange. Blinking only affects the white channel.

#### LED Strip Register Command Reference

To control each LED strip, write specific values to the control registers and blinking register. This table lists all required register addresses and values for each command:

| Name  | White ON | White ON Val | White OFF | WHITE OFF Val | Red ON | Red ON Val | Red OFF | Red OFF Val | Blink ON | Blink ON Val | Blink OFF | Blink OFF Val |
|-------|----------|--------------|-----------|---------------|--------|-----------|---------|-------------|----------|--------------|-----------|---------------|
| POWER | 0xA0     | 0x01         | 0xB0      | 0x01          | 0xA0   | 0x02      | 0xB0    | 0x02        | 0x50     | 0x01         | 0x50      | 0x00          |
| MGMT  | 0xA0     | 0x40         | 0xB0      | 0x40          | 0xA0   | 0x80      | 0xB0    | 0x80        | 0x56     | 0x01         | 0x56      | 0x00          |
| SATA1 | 0xA0     | 0x04         | 0xB0      | 0x04          | 0xA0   | 0x08      | 0xB0    | 0x08        | 0x52     | 0x01         | 0x52      | 0x00          |
| SATA2 | 0xA0     | 0x10         | 0xB0      | 0x10          | 0xA0   | 0x20      | 0xB0    | 0x20        | 0x54     | 0x01         | 0x54      | 0x00          |
| NVME1 | 0xA1     | 0x01         | 0xB1      | 0x01          | 0xA1   | 0x02      | 0xB1    | 0x02        | 0x58     | 0x01         | 0x58      | 0x00          |
| NVME2 | 0xA1     | 0x04         | 0xB1      | 0x04          | 0xA1   | 0x08      | 0xB1    | 0x08        | 0x5A     | 0x01         | 0x5A      | 0x00          |
| NVME3 | 0xA1     | 0x10         | 0xB1      | 0x10          | 0xA1   | 0x20      | 0xB1    | 0x20        | 0x5C     | 0x01         | 0x5C      | 0x00          |
| NVME4 | 0xA1     | 0x40         | 0xB1      | 0x40          | 0xA1   | 0x80      | 0xB1    | 0x80        | 0x5E     | 0x01         | 0x5E      | 0x00          |

**Usage Pattern:**
- To turn white ON: write the "White ON Val" to the "White ON" register
- To turn white OFF: write the "White OFF Val" to the "White OFF" register
- To turn red ON: write the "Red ON Val" to the "Red ON" register
- To turn red OFF: write the "Red OFF Val" to the "Red OFF" register
- To enable blinking: write the "Blink ON Val" to the "Blink ON" register
- To disable blinking: write the "Blink OFF Val" to the "Blink OFF" register

Each write is a single SMBus byte write to the specified register address on device 0x26.

### Communication Pattern

1. Open I2C device at `/dev/i2c-N` (bus number auto-detected or specified)
2. For LED bar: write mode (0x90), brightness (0x91), then RGB values (0x92-0x9A)
3. For LED strips: refer to the register command reference table above
4. All reads/writes go to device address 0x26

