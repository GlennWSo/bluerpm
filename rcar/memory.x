MEMORY
{
  /* https://github.com/lulf/microbit-bsp/blob/main/examples/ble-nrf-softdevice/memory.x */
MBR         : ORIGIN = 0x00000000, LENGTH = 4K
SOFTDEVICE  : ORIGIN = 0x00001000, LENGTH = 114688
FLASH       : ORIGIN = 0x0001C000, LENGTH = 405504
RAM         : ORIGIN = 0x2000afa8, LENGTH = 86104
}
