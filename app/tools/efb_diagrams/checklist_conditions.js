const CL_CONDITIONS = {
 "COCKPIT PREPARATION": {
  "SEAT BELTS": [
   {
    "v": "A:CABIN SEATBELTS ALERT SWITCH",
    "r": 1,
    "c": "EQ"
   }
  ],
  "GEAR PINS & COVERS": [
   {
    "v": "L:A32NX_WALKAROUND_PITOT_COVER_1",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_PITOT_COVER_2",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_PITOT_COVER_3",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_STATIC_COVERS",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_AOA_COVER_1",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_AOA_COVER_2",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_AOA_COVER_3",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_ENG_INLET_COVER_1",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_ENG_INLET_COVER_2",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_ENG_INLET_COVER_3",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_ENG_INLET_COVER_4",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_ENG_EXHAUST_COVER_1",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_ENG_EXHAUST_COVER_2",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_ENG_EXHAUST_COVER_3",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_ENG_EXHAUST_COVER_4",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_GEAR_PIN_NOSE",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_GEAR_PIN_LWING",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_GEAR_PIN_RWING",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_GEAR_PIN_LBODY",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_WALKAROUND_GEAR_PIN_RBODY",
    "r": 0,
    "c": "EQ"
   }
  ]
 },
 "BEFORE START": {
  "PARKING BRAKE": [
   {
    "v": "L:A32NX_PARK_BRAKE_LEVER_POS",
    "r": 1,
    "c": "EQ"
   }
  ],
  "BEACON": [
   {
    "v": "LIGHT BEACON",
    "r": 1,
    "c": "EQ"
   }
  ]
 },
 "TAXI": {
  "SEAT BELTS": [
   {
    "v": "A:CABIN SEATBELTS ALERT SWITCH",
    "r": 1,
    "c": "EQ"
   }
  ],
  "GND SPLRs": [
   {
    "v": "L:A32NX_SPOILERS_ARMED",
    "r": 1,
    "c": "EQ"
   }
  ],
  "AUTO BRK": [
   {
    "v": "L:A32NX_AUTOBRAKES_RTO_ARMED",
    "r": 1,
    "c": "EQ"
   }
  ],
  "FLAPS": [
   {
    "v": "L:A32NX_FLAPS_HANDLE_INDEX",
    "r": 1,
    "c": "GE"
   },
   {
    "v": "L:A32NX_FLAPS_HANDLE_INDEX",
    "r": 3,
    "c": "LE"
   }
  ]
 },
 "APPROACH": {
  "SEAT BELTS": [
   {
    "v": "A:CABIN SEATBELTS ALERT SWITCH",
    "r": 1,
    "c": "EQ"
   }
  ]
 },
 "LANDING": {
  "SEAT BELTS": [
   {
    "v": "A:CABIN SEATBELTS ALERT SWITCH",
    "r": 1,
    "c": "EQ"
   }
  ],
  "LDG GEAR": [
   {
    "v": "L:A32NX_GEAR_CENTER_SMALL_POSITION",
    "r": 100,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_GEAR_CENTER_POSITION",
    "r": 100,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_GEAR_LEFT_POSITION",
    "r": 100,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_GEAR_RIGHT_POSITION",
    "r": 100,
    "c": "EQ"
   }
  ],
  "GND SPLRs": [
   {
    "v": "L:A32NX_SPOILERS_ARMED",
    "r": 1,
    "c": "EQ"
   }
  ],
  "FLAPS": [
   {
    "v": "L:A32NX_FLAPS_HANDLE_INDEX",
    "r": 3,
    "c": "GE"
   }
  ]
 },
 "PARKING": {
  "FUEL PUMPs": [
   {
    "v": "A:CIRCUIT CONNECTION ON:75",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:77",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:76",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:79",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:78",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:74",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:73",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:72",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:71",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:70",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:69",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:68",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:67",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:66",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:65",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:64",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:3",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:2",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:81",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "A:CIRCUIT CONNECTION ON:80",
    "r": 0,
    "c": "EQ"
   }
  ],
  "PARKING BRAKE OR CHOCKS": [
   {
    "v": "L:A32NX_PARK_BRAKE_LEVER_POS",
    "r": 1,
    "c": "EQ"
   }
  ],
  "ENGINES": [
   {
    "v": "L:A32NX_ENGINE_STATE:1",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_ENGINE_STATE:2",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_ENGINE_STATE:3",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_ENGINE_STATE:4",
    "r": 0,
    "c": "EQ"
   }
  ]
 },
 "SECURING THE AIRCRAFT": {
  "OXYGEN": [
   {
    "v": "L:PUSH_OVHD_OXYGEN_CREW",
    "r": 1,
    "c": "EQ"
   }
  ],
  "EMER EXIT LIGHT": [
   {
    "v": "L:XMLVAR_SWITCH_OVHD_INTLT_EMEREXIT_POSITION",
    "r": 2,
    "c": "EQ"
   }
  ],
  "BATTERIES": [
   {
    "v": "L:A32NX_OVHD_ELEC_BAT_1_PB_IS_AUTO",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_OVHD_ELEC_BAT_ESS_PB_IS_AUTO",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_OVHD_ELEC_BAT_2_PB_IS_AUTO",
    "r": 0,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_OVHD_ELEC_BAT_APU_PB_IS_AUTO",
    "r": 0,
    "c": "EQ"
   }
  ]
 },
 "LINE-UP": {
  "PACK 1 & 2": [
   {
    "v": "L:A32NX_OVHD_COND_PACK_1_PB_IS_ON",
    "r": 1,
    "c": "EQ"
   },
   {
    "v": "L:A32NX_OVHD_COND_PACK_2_PB_IS_ON",
    "r": 1,
    "c": "EQ"
   }
  ]
 }
};
