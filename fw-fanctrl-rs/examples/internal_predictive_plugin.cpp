extern "C" {
#include "fw-fanctrl-rs.h"
}

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <string_view>
#include <cctype>

typedef long double FloatType;

// Configurable Constants for Kalman Filter
constexpr FloatType Q_NOISE_TEMP = 0.01;
constexpr FloatType Q_NOISE_VEL  = 0.1;
constexpr FloatType Q_NOISE_ACC  = 1.0;
constexpr FloatType Q_NOISE_JERK = 10.0;
constexpr FloatType R_MEASUREMENT_NOISE = 4.0; // The EC temp is discrete and noisy

constexpr FloatType PROJECTION_TIME_S = 3.0;    // How far into the future to project (seconds)
constexpr FloatType HYSTERESIS_UP_ALPHA = 0.8;      // Fast response when heating up
constexpr FloatType HYSTERESIS_DOWN_ALPHA = 0.05;   // Slow decay when cooling down

constexpr FloatType HEATSINK_TIME_CONSTANT_S = 30.0; // Seconds to reach ~63% of CPU temp (Newtonian Cooling)
constexpr FloatType SOAK_BASELINE_TEMP = 40.0;       // At what temp the heatsink is considered "soaked"
constexpr FloatType SOAK_EFFECT_MULTIPLIER = 1.5;    // How hard to push the fans when soaked

constexpr uint16_t MIN_POLL_MS = 50;
constexpr uint16_t MAX_POLL_MS = 1000;
constexpr FloatType POLL_VELOCITY_WEIGHT = 200.0;     // Subtracts this many ms per degree/sec
constexpr FloatType POLL_ACCELERATION_WEIGHT = 100.0; // Subtracts this many ms per degree/sec^2
constexpr FloatType POLL_JERK_WEIGHT = 50.0;          // Subtracts this many ms per degree/sec^3

struct PredictiveState {
  FloatType x[4];      // State Vector: [temp, vel, acc, jerk]
  FloatType p[4][4];   // Error Covariance matrix
  FloatType virtual_out_temp;
  FloatType estimated_heatsink_temp;
  uint64_t last_poll_time_ms;
  uint8_t cpu_sensor_idx;
  bool initialized;
};

// Helper for case-insensitive search
static bool contains_cpu_nocase(std::string_view str) {
  std::string_view target = "cpu";
  auto it = std::search(
    str.begin(), str.end(),
    target.begin(), target.end(),
    [](unsigned char ch1, unsigned char ch2) { return std::tolower(ch1) == std::tolower(ch2); }
  );
  return (it != str.end());
}

__attribute__((visibility("default"))) PluginDecision
get_decision(const PluginCallData *data) {
  const char *state_key = "predictive_state";
  PredictiveState state = {0};

  // Attempt to load the state
  bool has_state = GET_STATE(data, state_key, &state);

  FloatType current_temp_raw = (FloatType)ec_to_celsius(data->ancillary.highest_temp);

  // Error condition: invalid sensor data
  if (data->ancillary.num_sensors == 0 || data->ancillary.highest_temp == 0) {
    return MAKE_ERROR_SPEED(255);
  }

  // Handle first run initialization
  if (!has_state || !state.initialized) {
    state.x[0] = current_temp_raw;
    state.x[1] = 0.0;
    state.x[2] = 0.0;
    state.x[3] = 0.0;
    for(int i=0; i<4; i++) {
        for(int j=0; j<4; j++) {
            state.p[i][j] = (i == j) ? 10.0 : 0.0;
        }
    }
    state.virtual_out_temp = current_temp_raw;
    
    // Dynamically discover which sensor index is the CPU
    state.cpu_sensor_idx = 255; // Sentinel for "not found"
    for (uint8_t i = 0; i < data->ancillary.num_sensors; i++) {
       std::string_view name(data->sensors[i].sensor_name);
       if (contains_cpu_nocase(name)) {
           state.cpu_sensor_idx = i;
           break;
       }
    }
    
    FloatType cpu_temp = current_temp_raw;
    if (state.cpu_sensor_idx != 255) {
        cpu_temp = (FloatType)ec_to_celsius((*data->temps)[state.cpu_sensor_idx]);
    }
    state.estimated_heatsink_temp = cpu_temp;
    state.initialized = true;
    
    // Output current temp and save state
    SET_STATE(data, state_key, state);
    return make_curve_speed(celsius_to_ec((int16_t)std::clamp(current_temp_raw, (FloatType)0.0, (FloatType)105.0)), 0);
  }

  // Calculate time delta in seconds
  FloatType dt = (FloatType)data->ancillary.time_since_last_poll_ms / 1000.0;

  // Protect against divide by zero or extreme dt values
  if (dt <= 0.001) {
      dt = 0.001;
  } else if (dt > 5.0) {
      // If we've been asleep for a long time, reset the derivatives so we don't calculate an insane spike
      state.x[1] = 0.0;
      state.x[2] = 0.0;
      state.x[3] = 0.0;
      for(int i=0; i<4; i++) {
          for(int j=0; j<4; j++) {
              state.p[i][j] = (i == j) ? 10.0 : 0.0;
          }
      }
  }

  // --- KALMAN FILTER ---
  
  // 1. Prediction Step
  FloatType dt2 = dt * dt;
  FloatType dt3 = dt2 * dt;

  // State Transition Matrix (F)
  FloatType F[4][4] = {
      {static_cast<FloatType>(1.0), dt, static_cast<FloatType>(0.5) * dt2, static_cast<FloatType>(1.0/6.0) * dt3},
      {0.0, 1.0, dt, static_cast<FloatType>(0.5) * dt2},
      {0.0, 0.0, 1.0, dt},
      {0.0, 0.0, 0.0, 1.0}
  };

  FloatType x_pred[4];
  for(int i=0; i<4; i++) {
      x_pred[i] = 0.0;
      for(int j=0; j<4; j++) {
          x_pred[i] += F[i][j] * state.x[j];
      }
  }

  // Predict Covariance: P_pred = F * P * F^T + Q
  FloatType FP[4][4] = {0};
  for(int i=0; i<4; i++) {
      for(int j=0; j<4; j++) {
          for(int k=0; k<4; k++) {
              FP[i][j] += F[i][k] * state.p[k][j];
          }
      }
  }

  FloatType P_pred[4][4] = {0};
  for(int i=0; i<4; i++) {
      for(int j=0; j<4; j++) {
          for(int k=0; k<4; k++) {
              P_pred[i][j] += FP[i][k] * F[j][k]; // F^T means F[j][k]
          }
      }
  }
  
  // Add process noise Q
  P_pred[0][0] += Q_NOISE_TEMP * dt;
  P_pred[1][1] += Q_NOISE_VEL * dt;
  P_pred[2][2] += Q_NOISE_ACC * dt;
  P_pred[3][3] += Q_NOISE_JERK * dt;

  // 2. Update Step
  FloatType y = current_temp_raw - x_pred[0]; // Measurement residual
  FloatType S = P_pred[0][0] + R_MEASUREMENT_NOISE; // Residual covariance

  FloatType K[4]; // Kalman Gain
  for(int i=0; i<4; i++) {
      K[i] = P_pred[i][0] / S;
  }

  // Update State
  for(int i=0; i<4; i++) {
      state.x[i] = x_pred[i] + K[i] * y;
  }

  // Update Covariance P = (I - K H) * P_pred
  for(int i=0; i<4; i++) {
      for(int j=0; j<4; j++) {
          state.p[i][j] = P_pred[i][j] - K[i] * P_pred[0][j];
      }
  }

  // --- FORWARD PROJECTION ---
  // Kinematics equation: x = x0 + vt + 1/2at^2 + 1/6jt^3
  // We only project if velocity is positive (heating up). If cooling down, trust the current temp.
  FloatType projected_temp = state.x[0];
  if (state.x[1] > 0.0) {
      FloatType t = PROJECTION_TIME_S;
      FloatType t2 = t * t;
      FloatType t3 = t2 * t;
      projected_temp += (state.x[1] * t) + (0.5 * state.x[2] * t2) + (0.166667 * state.x[3] * t3);
  }
  
  // Bound the projected temp to reasonable values, and make sure we don't project *lower* than the actual smoothed temp if accelerating downwards rapidly
  projected_temp = std::max(projected_temp, state.x[0]);

  // --- THERMAL MASS (HEAT SOAK) INTEGRATION ---
  // Approximate Newtonian cooling: T(t) = T_env + (T_initial - T_env) * e^(-t/tau)
  // For standard discrete simulation, a dynamically integrated EMA works identically.
  FloatType alpha_heatsink = 1.0 - std::exp(-dt / HEATSINK_TIME_CONSTANT_S);
  
  FloatType cpu_temp_for_soak = current_temp_raw;
  if (state.cpu_sensor_idx != 255 && state.cpu_sensor_idx < data->ancillary.num_sensors) {
      cpu_temp_for_soak = (FloatType)ec_to_celsius((*data->temps)[state.cpu_sensor_idx]);
  }
  
  state.estimated_heatsink_temp = (alpha_heatsink * cpu_temp_for_soak) + ((1.0 - alpha_heatsink) * state.estimated_heatsink_temp);

  // Apply Heat Soak Penalty to prevent fans from spinning down while cooler is saturated
  FloatType thermal_overhead = state.estimated_heatsink_temp - SOAK_BASELINE_TEMP;
  if (thermal_overhead > 0.0) {
      projected_temp += (thermal_overhead * SOAK_EFFECT_MULTIPLIER);
  }

  // --- ASYMMETRICAL HYSTERESIS FILTER ---
  if (projected_temp > state.virtual_out_temp) {
      // Ramping up: Fast response
      state.virtual_out_temp = (HYSTERESIS_UP_ALPHA * projected_temp) + ((1.0 - HYSTERESIS_UP_ALPHA) * state.virtual_out_temp);
  } else {
      // Ramping down: Slow decay to prevent oscillation
      state.virtual_out_temp = (HYSTERESIS_DOWN_ALPHA * projected_temp) + ((1.0 - HYSTERESIS_DOWN_ALPHA) * state.virtual_out_temp);
  }

  // Final bounds checking (Celsius: 0 to 105C)
  uint8_t final_ec_target_temp = celsius_to_ec((int16_t)std::clamp(state.virtual_out_temp, (FloatType)0.0, (FloatType)105.0));

  // --- DYNAMIC POLLING CALCULATIONS ---
  FloatType velocity_mag = std::abs(state.x[1]);
  FloatType accel_mag = std::abs(state.x[2]);
  FloatType jerk_mag = std::abs(state.x[3]);
  
  // The system should wake up sooner if the temperature is changing rapidly
  FloatType poll_reduction = (velocity_mag * POLL_VELOCITY_WEIGHT) + (accel_mag * POLL_ACCELERATION_WEIGHT) + (jerk_mag * POLL_JERK_WEIGHT);
  FloatType calculated_poll = (FloatType)MAX_POLL_MS - poll_reduction;
  
  uint16_t dynamic_poll_ms = (uint16_t)std::clamp(calculated_poll, (FloatType)MIN_POLL_MS, (FloatType)MAX_POLL_MS);

  // Save state for next poll
  SET_STATE(data, state_key, state);

  return make_curve_speed(final_ec_target_temp, dynamic_poll_ms);
}
