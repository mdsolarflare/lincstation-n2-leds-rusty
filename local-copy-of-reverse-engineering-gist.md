This is sourced in it's original form (typos and all) from https://gist.github.com/ffalt/984aa3644a90d4230eaf5b129aaf1eeb duplicated only for providing credit and preserving the text.
> [!WARNING]
> i2cset commands with wrong parameters can damage your system. Handle with care!
>
> The bus number may change after a restart, don't hardcode it into your scripts.

Before you use any commands on this page you MUST find out the bus number for the led control on your system

Run
```
i2cdetect -y 0

i2cdetect -y 1

i2cdetect -y 2

i2cdetect -y 3

i2cdetect -y 4
```

until you see something on 26

```
>i2cdetect -y 1
     0  1  2  3  4  5  6  7  8  9  a  b  c  d  e  f
00:                         08 -- -- -- -- -- -- --
10: -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- --
20: -- -- -- -- -- -- 26 -- -- -- -- -- -- -- -- --
30: -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- --
40: -- -- -- -- 44 -- -- -- -- -- -- -- -- -- -- --
50: -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- --
60: -- -- -- -- -- -- -- -- -- -- -- -- -- -- -- --
70: -- -- -- -- -- -- -- --
```
This is the -y <bus> parameter you must use.
In this example it is `1`
A full command would be
```
# LED bar example: off (set mode to solid, set brightness to 0)
i2cset -y 1 0x26 0x90 0 
i2cset -y 1 0x26 0x91 0
```

## LED bar
```  
# LED bar modes (0 = solid, 1 = breath, 2 = loop)
i2cset -y <bus> 0x26 0x90 0

# LED bar brightness (0-255)
i2cset -y <bus> 0x26 0x91 1 

# LED bar LOOP COLOR 1 BYTE 1 (0-255)
i2cset -y <bus> 0x26 0x95 0

# LED bar LOOP COLOR 1 BYTE 2 (0-255)
i2cset -y <bus> 0x26 0x96 0

# LED bar LOOP COLOR 1 BYTE 3 (0-255)
i2cset -y <bus> 0x26 0x97 0 

# LED bar LOOP COLOR 2 BYTE 1 (0-255)
i2cset -y <bus> 0x26 0x95 0

# LED bar LOOP COLOR 2 BYTE 2 (0-255)
i2cset -y <bus> 0x26 0x96 0

# LED bar LOOP COLOR 2 BYTE 3 (0-255)
i2cset -y <bus> 0x26 0x97 0

# LED bar BREATH COLOR BYTE 1 (0-255)
i2cset -y <bus> 0x26 0x92 0 

# LED bar BREATH COLOR BYTE 2 (0-255)
i2cset -y <bus> 0x26 0x93 0 

# LED bar BREATH COLOR BYTE 3 (0-255)
i2cset -y <bus> 0x26 0x94 0
```

## LED bar examples
```  
# LED bar example: off (set mode to solid, set brightness to 0)
i2cset -y <bus> 0x26 0x90 0 
i2cset -y <bus> 0x26 0x91 0

# LED bar example: solid
i2cset -y <bus> 0x26 0x90 0
i2cset -y <bus> 0x26 0x91 128
i2cset -y <bus> 0x26 0x92 255
i2cset -y <bus> 0x26 0x93 204
i2cset -y <bus> 0x26 0x94 102

# LED bar example: solid red
i2cset -y <bus> 0x26 0x90 0
i2cset -y <bus> 0x26 0x91 255
i2cset -y<bus>1 0x26 0x92 255
i2cset -y <bus> 0x26 0x93 0
i2cset -y <bus> 0x26 0x94 0

# LED bar example: breath
i2cset -y <bus> 0x26 0x90 1
i2cset -y <bus> 0x26 0x91 255
i2cset -y <bus> 0x26 0x92 102
i2cset -y <bus> 0x26 0x93 204
i2cset -y <bus> 0x26 0x94 255

# LED bar example: loop
i2cset -y <bus> 0x26 0x90 2
i2cset -y <bus> 0x26 0x91 255
i2cset -y <bus> 0x26 0x95 255
i2cset -y <bus> 0x26 0x96 0
i2cset -y <bus> 0x26 0x97 0
i2cset -y <bus> 0x26 0x98 126
i2cset -y <bus> 0x26 0x99 204
i2cset -y <bus> 0x26 0x9A 255

# LED bar example: loop green
i2cset -y <bus> 0x26 0x90 2
i2cset -y <bus> 0x26 0x91 255
i2cset -y <bus> 0x26 0x95 0
i2cset -y <bus> 0x26 0x96 255
i2cset -y <bus> 0x26 0x97 255
i2cset -y <bus> 0x26 0x98 255
i2cset -y <bus> 0x26 0x99 255
i2cset -y <bus> 0x26 0x9A 0
```  

## Switch button LED

```  
# LED Switch on
i2cset -y <bus> 0x26 0XA0 0x01

# LED Switch off
i2cset -y <bus> 0x26 0XB0 0x01

# LED Switch red on
i2cset -y <bus> 0x26 0XA0 0x02

# LED Switch red off
i2cset -y <bus> 0x26 0XB0 0x02

# LED Switch blinking on
i2cset -y <bus> 0x26 0x50 0x01

# LED Switch blinking off
i2cset -y <bus> 0x26 0x50 0x00
```  

## LEDs strip

```  
# STRIP HDD 0 on
i2cset -y <bus> 0x26 0XA0 0x04

# STRIP HDD 0 off
i2cset -y <bus> 0x26 0XB0 0x04

# STRIP HDD 0 red on
i2cset -y <bus> 0x26 0XA0 0x08

# STRIP HDD 0 red off
i2cset -y <bus> 0x26 0XB0 0x08

# STRIP HDD 0 blinking on
i2cset -y <bus> 0x26 0x52 0x01

# STRIP HDD 0 blinking off
i2cset -y <bus> 0x26 0x52 0x00


# STRIP HDD 1 on
i2cset -y <bus> 0x26 0XA0 0x10

# STRIP HDD 1 off
i2cset -y <bus> 0x26 0XB0 0x10

# STRIP HDD 1 red on
i2cset -y <bus> 0x26 0XA0 0x20

# STRIP HDD 1 red off
i2cset -y <bus> 0x26 0XB0 0x20

# STRIP HDD 1 blinking on
i2cset -y <bus> 0x26 0x54 0x01

# STRIP HDD 1 blinking off
i2cset -y <bus> 0x26 0x54 0x00



# STRIP network on
i2cset -y <bus> 0x26 0XA0 0x40

# STRIP network off
i2cset -y <bus> 0x26 0XB0 0x40

# STRIP network red on
i2cset -y <bus> 0x26 0XA0 0x80

# STRIP network red off
i2cset -y <bus> 0x26 0XB0 0x80

# STRIP network blinking on
i2cset -y <bus> 0x26 0x56 0x01

# STRIP network blinking off
i2cset -y <bus> 0x26 0x56 0x00


# STRIP NVME 1 on
i2cset -y <bus> 0x26 0XA1 0x01

# STRIP NVME 1 off
i2cset -y <bus> 0x26 0XB1 0x01

# STRIP NVME 1 red on
i2cset -y <bus> 0x26 0XA1 0x02

# STRIP NVME 1 red off
i2cset -y <bus> 0x26 0XB1 0x02

# STRIP NVME 1 blinking on
i2cset -y <bus> 0x26 0x58 0x01

# STRIP NVME 1 blinking off
i2cset -y <bus> 0x26 0x58 0x00


# STRIP NVME 2 on
i2cset -y <bus> 0x26 0XA1 0x04

# STRIP NVME 2 off
i2cset -y <bus> 0x26 0XB1 0x04
 
# STRIP NVME 2 red on
i2cset -y <bus> 0x26 0XA1 0x08

# STRIP NVME 2 red off
i2cset -y <bus> 0x26 0XB1 0x08

# STRIP NVME 2 blinking on
i2cset -y <bus> 0x26 0x5A 0x01

# STRIP NVME 2 blinking off
i2cset -y <bus> 0x26 0x5A 0x00


# STRIP NVME 3 on
i2cset -y <bus> 0x26 0XA1 0x10

# STRIP NVME 3 off
i2cset -y <bus> 0x26 0XB1 0x10

# STRIP NVME 3 red on
i2cset -y <bus> 0x26 0XA1 0x20

# STRIP NVME 3 red off
i2cset -y <bus> 0x26 0XB1 0x20

# STRIP NVME 3 blinking on
i2cset -y <bus> 0x26 0x5C 0x01

# STRIP NVME 3 blinking off
i2cset -y <bus> 0x26 0x5C 0x00


# STRIP NVME 4 on
i2cset -y <bus> 0x26 0XA1 0x40

# STRIP NVME 4 off
i2cset -y <bus> 0x26 0XB1 0x40

# STRIP NVME 4 red on
i2cset -y <bus> 0x26 0XA1 0x80

# STRIP NVME 4 red off
i2cset -y <bus> 0x26 0XB1 0x80

# STRIP NVME 4 blinking on
i2cset -y <bus> 0x26 0x5E 0x01

# STRIP NVME 4 blinking off
i2cset -y <bus> 0x26 0x5E 0x00
```

## LEDs strip examples
```
# Turn off blinking (and turn off red/white lights that where on while we turned off blinking )
i2cset -y <bus> 0x26 0x52 0x00
i2cset -y <bus> 0x26 0x54 0x00
i2cset -y <bus> 0x26 0x56 0x00
i2cset -y <bus> 0x26 0x58 0x00
i2cset -y <bus> 0x26 0x5A 0x00
i2cset -y <bus> 0x26 0x5C 0x00
i2cset -y <bus> 0x26 0x5E 0x00

i2cset -y <bus> 0x26 0XB0 0x04
i2cset -y <bus> 0x26 0XB0 0x08
i2cset -y <bus> 0x26 0XB0 0x10
i2cset -y <bus> 0x26 0XB0 0x20
i2cset -y <bus> 0x26 0XB0 0x40
i2cset -y <bus> 0x26 0XB0 0x80
i2cset -y <bus> 0x26 0XB1 0x01
i2cset -y <bus> 0x26 0XB1 0x02
i2cset -y <bus> 0x26 0XB1 0x04
i2cset -y <bus> 0x26 0XB1 0x08
i2cset -y <bus> 0x26 0XB1 0x10
i2cset -y <bus> 0x26 0XB1 0x20
i2cset -y <bus> 0x26 0XB1 0x40
i2cset -y <bus> 0x26 0XB1 0x80
```
