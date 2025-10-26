#include <linux/gpio.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/ioctl.h>
#include <string.h>
#include <time.h>

static void
addms(struct timespec *t, int ms)
{
  int ns = ms * 1000000;

  t->tv_nsec += ns;
  if(t->tv_nsec > 1000000000) {
    t->tv_nsec -= 1000000000;
    t->tv_sec++;
  }
}

static void
addus(struct timespec *t, int us)
{
  int ns = us * 1000;
  t->tv_nsec += ns;
  if(t->tv_nsec > 1000000000) {
    t->tv_nsec -= 1000000000;
    t->tv_sec++;
  }
}


static void
set_line(int fd, int value, int delay, struct timespec *t)
{
  struct gpiohandle_data data = {};
  data.values[0] = value;
  ioctl(fd, GPIOHANDLE_SET_LINE_VALUES_IOCTL, &data);
  addus(t, delay);
  clock_nanosleep(CLOCK_REALTIME, TIMER_ABSTIME, t, NULL);
}

#define HVAC_MITSUBISHI_HDR_MARK   3400
#define HVAC_MITSUBISHI_HDR_SPACE  1750
#define HVAC_MITSUBISHI_BIT_MARK   340
#define HVAC_MITSUBISHI_ONE_SPACE  1300
#define HVAC_MITSUBISHI_ZERO_SPACE 420
#define HVAC_MITSUBISHI_RPT_MARK   440
#define HVAC_MITSUBISHI_RPT_SPACE  17100


static void
send_byte(int fd, uint8_t byte, struct timespec *t)
{
  printf("byte:%x\n", byte);
  for(size_t i = 0; i < 8; i++) {
    set_line(fd, 1, HVAC_MITSUBISHI_BIT_MARK, t);
    set_line(fd, 0, (1 << i) & byte ?
	     HVAC_MITSUBISHI_ONE_SPACE :
	     HVAC_MITSUBISHI_ZERO_SPACE, t);
  }
}


static void
send_msg(int fd, const uint8_t msg[static 18], struct timespec *t)
{
  for(size_t i = 0; i < 18; i++) {
    send_byte(fd, msg[i], t);
  }
}


union payload {
  uint8_t data[18];

  struct {
    uint8_t magic[5];
    uint8_t onoff;
    uint8_t hvac_mode;
    uint8_t temperature;
    uint8_t hvac_mode2;
    uint8_t fan_speed;
    uint8_t clock;
    uint8_t endclock;
    uint8_t startclock;
    uint8_t progmode;
    uint8_t zero[3];
    uint8_t checksum;
  };
};



int
main(void)
{
  int fd = open("/dev/gpiochip0", O_RDWR);
  if(fd == -1) {
    perror("gpio");
    exit(1);
  }

  struct gpiochip_info cinfo;
  int ret = ioctl(fd, GPIO_GET_CHIPINFO_IOCTL, &cinfo);
  fprintf(stdout, "GPIO chip: %s, \"%s\", %u GPIO lines\n",
          cinfo.name, cinfo.label, cinfo.lines);

  for(int i = 0; i < cinfo.lines; i++) {
    struct gpioline_info linfo;
    linfo.line_offset = i;
    ret = ioctl(fd, GPIO_GET_LINEINFO_IOCTL, &linfo);
    if(ret)
      break;
    fprintf(stdout, "line %3d: 0x%02x %s %s\n",
            linfo.line_offset, linfo.flags, linfo.name, linfo.consumer);
  }

  
  struct gpiohandle_request req = {};

  req.lineoffsets[0] = 4;
  req.default_values[0] = 0;
  req.lines = 1;
  req.flags = GPIOHANDLE_REQUEST_OUTPUT;
  strcpy(req.consumer_label, "AC");

  int r = ioctl(fd, GPIO_GET_LINEHANDLE_IOCTL, &req);
  if(r) {
    perror("GPIO_GET_LINEHANDLE_IOCTL");
  }

  int line_fd = req.fd;
  printf("fd=%d\n", req.fd);

  struct gpiohandle_data data = {};

  struct timespec t;

  union payload p;
  memset(&p, 0, sizeof(p));


  p.magic[0] = 0x23;
  p.magic[1] = 0xcb;
  p.magic[2] = 0x26;
  p.magic[3] = 0x01;
  p.magic[4] = 0x00;
  p.onoff = 0x20;
  p.hvac_mode = 0x8;
  p.temperature = 0x5;
  p.hvac_mode2 = 0x30;
  p.fan_speed = 0x63;

  uint8_t acc = 0;
  for(int i = 0; i < 17; i++) {
    acc += p.data[i];
  }
  p.checksum = acc;
  

  clock_gettime(CLOCK_REALTIME, &t);

  set_line(line_fd, 1, HVAC_MITSUBISHI_HDR_MARK, &t);
  set_line(line_fd, 0, HVAC_MITSUBISHI_HDR_SPACE, &t);

  send_msg(line_fd, p.data, &t);

  set_line(line_fd, 1, HVAC_MITSUBISHI_RPT_MARK, &t);
  set_line(line_fd, 0, HVAC_MITSUBISHI_RPT_SPACE, &t);

  set_line(line_fd, 1, HVAC_MITSUBISHI_HDR_MARK, &t);
  set_line(line_fd, 0, HVAC_MITSUBISHI_HDR_SPACE, &t);

  send_msg(line_fd, p.data, &t);

  
}
