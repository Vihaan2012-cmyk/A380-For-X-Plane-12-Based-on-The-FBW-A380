import json
from pathlib import Path

OUT = Path(__file__).parent / "scenarios"


def press(rmp, key, hold=50):
    step = {"press": f"RMP_{rmp}_{key}"}
    if hold != 50:
        step["hold_ms"] = hold
    return step


def digits(rmp, text):
    return [press(rmp, "DIGIT_DOT" if c == "." else f"DIGIT_{c}") for c in text]


def run(ms):
    return {"run_ms": ms}


def put(**kw):
    return {"set": {k.replace("__", ":"): v for k, v in kw.items()}}


SC = {}
SC["power_up"] = [run(2000)]
SC["vhf_entry_transfer"] = (
    digits(1, "119000") + [press(1, "LSK_1"), run(500), press(1, "ADK_1"), run(500)]
    + digits(1, "121500") + [press(1, "LSK_2"), run(300), press(1, "ADK_2"), run(500)]
    + digits(2, "13") + [press(2, "DIGIT_CLR"), press(2, "DIGIT_CLR"), run(300)]
    + digits(1, "120123") + [press(1, "LSK_1"), run(600), press(1, "MSG_CLR"), run(500), press(1, "ADK_1"), run(500)]
)
SC["incomplete_freq_message"] = [press(1, "LSK_1"), run(600)] + digits(1, "118") + [press(1, "LSK_1"), run(600), press(1, "MSG_CLR"), run(800)] + digits(1, "11") + [run(11000)]
SC["vhf3_data_emer"] = [press(1, "LSK_3"), press(1, "UP"), press(1, "UP"), run(300), press(1, "DOWN"), run(300), press(1, "UP"), press(1, "UP"), press(1, "ADK_3"), run(500), press(1, "ADK_3"), run(500)]
SC["squawk"] = (
    [press(1, "SQWK"), run(300)] + digits(1, "7700") + [run(1500)]
    + digits(1, "8") + [run(300)] + digits(1, "12") + [press(1, "DIGIT_CLR"), run(400)]
    + digits(1, "3456") + [run(1500), press(1, "ADK_3"), run(1000), press(1, "ADK_3"), run(6000), press(1, "VHF"), run(300)]
)
SC["transmit_receive"] = (
    [press(1, "VHF_CALL_1"), run(300), press(1, "VHF_CALL_2"), run(300), press(1, "HF_CALL_1"), run(300), press(1, "VOICE"), run(300), press(1, "VOICE"), press(1, "MECH_CALL"), run(300), press(1, "MECH_CALL"), run(300),
     put(L__A380X_RMP_1_VHF_VOL_RX_SWITCH_1=1), run(600), put(L__A380X_RMP_1_VHF_VOL_RX_SWITCH_1=0), run(300), put(L__A380X_RMP_1_VHF_VOL_RX_SWITCH_1=1), run(100), put(L__A380X_RMP_1_VHF_VOL_RX_SWITCH_1=0), run(600),
     press(1, "VHF_CALL_1"), run(600), put(L__A380X_RMP_1_VHF_VOL_RX_SWITCH_1=1), run(600), press(1, "VHF_CALL_1"), run(1500), press(1, "TEL_CALL_1"), run(800), press(1, "TEL_CALL_1"), run(300)]
)
SC["rmp_failure"] = [
    run(500), {"failures": [23002]}, run(1000), press(1, "LSK_1"), run(500), {"failures": []}, run(1000),
    put(L__A380X_RMP_2_BRIGHTNESS_KNOB=0), run(500), put(L__A380X_RMP_2_BRIGHTNESS_KNOB=50), run(500),
]
SC["rmp3_backup"] = (
    digits(3, "125350") + [press(3, "LSK_1"), run(300), press(3, "ADK_1"), run(500),
    put(L__A32NX_ELEC_DC_ESS_BUS_IS_POWERED=0, L__A32NX_ELEC_DC_1_BUS_IS_POWERED=1), run(1500),
    put(L__A32NX_ELEC_DC_ESS_BUS_IS_POWERED=1), run(1500), put(L__A32NX_ELEC_DC_1_BUS_IS_POWERED=0), run(1500),
    put(L__A32NX_ELEC_DC_1_BUS_IS_POWERED=1), run(1000)]
)
for name, steps in SC.items():
    (OUT / f"{name}.json").write_text(json.dumps({"name": name, "steps": steps}, indent=1) + "\n", encoding="utf-8")
print(len(SC), "scenarios")
