// LIS2MDL magnetometer demo for the Wokwi web editor (Arduino, ESP32-C3).
//
// Talks to the LIS2MDL Wokwi chip over raw I2C and prints field, heading, temperature and
// threshold interrupts in the same format as the Rust example in ../esp32c3.
// Wiring: SDA = GPIO4, SCL = GPIO5, INT/DRDY = GPIO7.

#include <Wire.h>

constexpr uint8_t LIS2MDL_ADDRESS = 0x1E;
constexpr uint8_t LIS2MDL_WHO_AM_I = 0x40;

// Set in the register address (SUB) for multi-byte transfers.
constexpr uint8_t I2C_AUTO_INCREMENT = 0x80;

constexpr uint8_t REG_OFFSET_X_REG_L = 0x45;
constexpr uint8_t REG_WHO_AM_I = 0x4F;
constexpr uint8_t REG_CFG_REG_A = 0x60;
constexpr uint8_t REG_CFG_REG_B = 0x61;
constexpr uint8_t REG_CFG_REG_C = 0x62;
constexpr uint8_t REG_INT_CRTL_REG = 0x63;
constexpr uint8_t REG_INT_SOURCE_REG = 0x64;
constexpr uint8_t REG_INT_THS_L_REG = 0x65;
constexpr uint8_t REG_STATUS_REG = 0x67;
constexpr uint8_t REG_OUTX_L_REG = 0x68;
constexpr uint8_t REG_TEMP_OUT_L_REG = 0x6E;

// CFG_REG_A: COMP_TEMP_EN, REBOOT, SOFT_RST, ODR[1:0], MD[1:0] (continuous = 00).
constexpr uint8_t CFG_A_COMP_TEMP_EN = 0x80;
constexpr uint8_t CFG_A_REBOOT = 0x40;
constexpr uint8_t CFG_A_SOFT_RST = 0x20;
constexpr uint8_t CFG_A_ODR_MASK = 0x0C;
constexpr uint8_t CFG_A_ODR_50HZ = 0x08;
constexpr uint8_t CFG_A_ODR_100HZ = 0x0C;
constexpr uint8_t CFG_A_MD_MASK = 0x03;

// CFG_REG_B: OFF_CANC.
constexpr uint8_t CFG_B_OFF_CANC = 0x02;

// CFG_REG_C: INT_ON_PIN, BDU, SELF_TEST.
constexpr uint8_t CFG_C_INT_ON_PIN = 0x40;
constexpr uint8_t CFG_C_BDU = 0x10;
constexpr uint8_t CFG_C_SELF_TEST = 0x02;

// INT_CRTL_REG: XIEN, YIEN, ZIEN (reset defaults), IEA (active high), IEN.
constexpr uint8_t INT_CTRL_XYZ_IEN = 0xE0;
constexpr uint8_t INT_CTRL_IEA = 0x04;
constexpr uint8_t INT_CTRL_IEN = 0x01;

// STATUS_REG: Zyxda.
constexpr uint8_t STATUS_ZYXDA = 0x08;

constexpr float MILLIGAUSS_PER_LSB = 1.5f;
constexpr float TEMPERATURE_LSB_PER_CELSIUS = 8.0f;
constexpr float TEMPERATURE_OFFSET_CELSIUS = 25.0f;
constexpr uint32_t TURN_ON_TIME_MS = 10;

// Self-test limits and timing (DS12144 Rev 6, section 6.3).
constexpr float SELF_TEST_MIN_MILLIGAUSS = 15.0f;
constexpr float SELF_TEST_MAX_MILLIGAUSS = 500.0f;
constexpr int SELF_TEST_SAMPLES = 50;

// The hard-iron offset written to the device: 15 mG on X.
constexpr float HARD_IRON_OFFSET_X_MG = 15.0f;

// The threshold interrupt fires when any axis exceeds ±1 gauss.
constexpr float THRESHOLD_MG = 1000.0f;

constexpr int PIN_SDA = 4;
constexpr int PIN_SCL = 5;
constexpr int PIN_INT = 7;

uint8_t sub(uint8_t reg, size_t len) {
  return len > 1 ? reg | I2C_AUTO_INCREMENT : reg;
}

bool readRegisters(uint8_t reg, uint8_t *buffer, size_t len) {
  Wire.beginTransmission(LIS2MDL_ADDRESS);
  Wire.write(sub(reg, len));
  if (Wire.endTransmission(false) != 0) {
    return false;
  }

  if (Wire.requestFrom(int(LIS2MDL_ADDRESS), int(len)) != int(len)) {
    return false;
  }

  for (size_t i = 0; i < len; i++) {
    buffer[i] = Wire.read();
  }

  return true;
}

uint8_t readRegister(uint8_t reg) {
  uint8_t value = 0;
  readRegisters(reg, &value, 1);
  return value;
}

void writeRegisters(uint8_t reg, const uint8_t *data, size_t len) {
  Wire.beginTransmission(LIS2MDL_ADDRESS);
  Wire.write(sub(reg, len));
  Wire.write(data, len);
  Wire.endTransmission();
}

void writeRegister(uint8_t reg, uint8_t value) {
  writeRegisters(reg, &value, 1);
}

bool dataReady() {
  return readRegister(REG_STATUS_REG) & STATUS_ZYXDA;
}

// X, Y, Z as little-endian 16-bit raw counts.
bool readField(int16_t raw[3]) {
  uint8_t data[6];
  if (!readRegisters(REG_OUTX_L_REG, data, sizeof(data))) {
    return false;
  }

  for (int i = 0; i < 3; i++) {
    raw[i] = int16_t(uint16_t(data[2 * i + 1]) << 8 | data[2 * i]);
  }

  return true;
}

bool readTemperature(int16_t *raw) {
  uint8_t data[2];
  if (!readRegisters(REG_TEMP_OUT_L_REG, data, sizeof(data))) {
    return false;
  }

  *raw = int16_t(uint16_t(data[1]) << 8 | data[0]);
  return true;
}

bool waitForData() {
  for (int i = 0; i < 100; i++) {
    if (dataReady()) {
      return true;
    }
    delay(1);
  }

  return false;
}

// Sums SELF_TEST_SAMPLES samples after discarding the first one.
bool selfTestSum(int32_t sum[3]) {
  int16_t raw[3];
  if (!waitForData() || !readField(raw)) {
    return false;
  }

  sum[0] = sum[1] = sum[2] = 0;
  for (int n = 0; n < SELF_TEST_SAMPLES; n++) {
    if (!waitForData() || !readField(raw)) {
      return false;
    }
    for (int i = 0; i < 3; i++) {
      sum[i] += raw[i];
    }
  }

  return true;
}

// Self-test as in DS12144 Rev 6, section 6.3; restores the configuration afterwards.
bool selfTest(float difference[3]) {
  uint8_t savedA = readRegister(REG_CFG_REG_A);
  uint8_t savedB = readRegister(REG_CFG_REG_B);
  uint8_t savedC = readRegister(REG_CFG_REG_C);

  uint8_t c = (savedC | CFG_C_BDU) & ~CFG_C_SELF_TEST;
  writeRegister(REG_CFG_REG_C, c);
  writeRegister(REG_CFG_REG_B, CFG_B_OFF_CANC);
  writeRegister(REG_CFG_REG_A, CFG_A_COMP_TEMP_EN | CFG_A_ODR_100HZ);
  delay(20);

  int32_t without[3], with[3];
  bool ok = selfTestSum(without);
  if (ok) {
    writeRegister(REG_CFG_REG_C, c | CFG_C_SELF_TEST);
    delay(60);
    ok = selfTestSum(with);
  }

  writeRegister(REG_CFG_REG_C, savedC);
  writeRegister(REG_CFG_REG_B, savedB);
  writeRegister(REG_CFG_REG_A, savedA);

  for (int i = 0; ok && i < 3; i++) {
    difference[i] = fabsf(float(with[i] - without[i]) / SELF_TEST_SAMPLES * MILLIGAUSS_PER_LSB);
  }

  return ok;
}

void halt() {
  while (true) {
    delay(1000);
  }
}

void setup() {
  Serial.begin(115200);

  pinMode(PIN_INT, INPUT_PULLDOWN);
  Wire.begin(PIN_SDA, PIN_SCL);
  Wire.setClock(400000);

  // The device needs its turn-on time after power-up.
  delay(TURN_ON_TIME_MS);

  uint8_t whoAmI = 0;
  if (!readRegisters(REG_WHO_AM_I, &whoAmI, 1) || whoAmI != LIS2MDL_WHO_AM_I) {
    Serial.printf("LIS2MDL not found (WHO_AM_I 0x%02X)\n", whoAmI);
    halt();
  }

  Serial.printf("LIS2MDL ok who_am_i=0x%02X\n", whoAmI);

  float d[3];
  if (!selfTest(d)) {
    Serial.println("self-test failed");
    halt();
  }

  bool passed = true;
  for (float v : d) {
    passed &= v >= SELF_TEST_MIN_MILLIGAUSS && v <= SELF_TEST_MAX_MILLIGAUSS;
  }

  Serial.printf("self-test passed=%s dx=%.1f dy=%.1f dz=%.1f\n", passed ? "true" : "false",
                d[0], d[1], d[2]);

  // Hard-iron offset in LSB, rounded to nearest: X, Y, Z little-endian.
  int16_t offsetX = int16_t(HARD_IRON_OFFSET_X_MG / MILLIGAUSS_PER_LSB + 0.5f);
  uint8_t offset[6] = {uint8_t(offsetX), uint8_t(offsetX >> 8), 0, 0, 0, 0};
  writeRegisters(REG_OFFSET_X_REG_L, offset, sizeof(offset));

  uint16_t threshold = uint16_t(THRESHOLD_MG / MILLIGAUSS_PER_LSB);
  uint8_t thresholdBytes[2] = {uint8_t(threshold), uint8_t(threshold >> 8)};
  writeRegisters(REG_INT_THS_L_REG, thresholdBytes, sizeof(thresholdBytes));

  // Pulsed, active high: INT follows the current sample.
  writeRegister(REG_INT_CRTL_REG, INT_CTRL_XYZ_IEN | INT_CTRL_IEA | INT_CTRL_IEN);
  writeRegister(REG_CFG_REG_C, readRegister(REG_CFG_REG_C) | CFG_C_INT_ON_PIN | CFG_C_BDU);

  // Continuous mode at 50 Hz with temperature compensation.
  uint8_t a = readRegister(REG_CFG_REG_A);
  a &= ~(CFG_A_REBOOT | CFG_A_SOFT_RST | CFG_A_ODR_MASK | CFG_A_MD_MASK);
  writeRegister(REG_CFG_REG_A, a | CFG_A_COMP_TEMP_EN | CFG_A_ODR_50HZ);
  Serial.printf("running odr=50Hz threshold=%.0fmG\n", THRESHOLD_MG);
}

void loop() {
  static int16_t last[4];
  static bool haveLast = false;
  static bool intActive = false;

  // X, Y, Z and temperature.
  int16_t raw[4];
  if (dataReady() && readField(raw) && readTemperature(&raw[3])) {
    if (!haveLast || memcmp(raw, last, sizeof(raw)) != 0) {
      float x = raw[0] * MILLIGAUSS_PER_LSB;
      float y = raw[1] * MILLIGAUSS_PER_LSB;
      float z = raw[2] * MILLIGAUSS_PER_LSB;

      // Heading in degrees (0–360) clockwise from magnetic north, for a level board with
      // the chip lying flat: X points north, Y east and Z up.
      float heading = atan2f(y, x) * 180.0f / PI;
      if (heading < 0) {
        heading += 360.0f;
      }

      float celsius = raw[3] / TEMPERATURE_LSB_PER_CELSIUS + TEMPERATURE_OFFSET_CELSIUS;
      Serial.printf("mag x=%.1f y=%.1f z=%.1f mG heading=%.0f temp=%.1fC\n", x, y, z, heading,
                    celsius);
      memcpy(last, raw, sizeof(raw));
      haveLast = true;
    }
  }

  bool level = digitalRead(PIN_INT) == HIGH;
  if (level && !intActive) {
    Serial.printf("int src=0x%02X\n", readRegister(REG_INT_SOURCE_REG));
  } else if (!level && intActive) {
    Serial.println("int clear");
  }

  intActive = level;

  delay(5);
}
