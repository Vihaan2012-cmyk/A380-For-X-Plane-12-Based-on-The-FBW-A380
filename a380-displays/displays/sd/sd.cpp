#include <algorithm>
#include "a380/shape_cache.h"
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <map>
#include <string>

#include <vector>

#include "include/codec/SkCodec.h"
#include "include/core/SkBitmap.h"
#include "include/core/SkCanvas.h"
#include "include/core/SkData.h"
#include "include/core/SkImage.h"
#include "include/core/SkMatrix.h"
#include "include/core/SkMaskFilter.h"
#include "include/core/SkBlurTypes.h"
#include "include/core/SkSamplingOptions.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPath.h"
#include "include/core/SkPathBuilder.h"

#include "../sdsys/sdsys.h"
#include "a380/assets.h"
#include "a380/display.h"
#include "a380/display_unit.h"
#include "a380/draw.h"
#include "a380/fonts.h"
#include "a380/raster.h"

namespace {

using a380::Align;
using a380::rgb;

constexpr uint32_t kWhite = 0xffffff, kGreen = 0x00ff00, kAmber = 0xe68000, kCyan = 0x00ffff, kRed = 0xff0000;

struct Arinc {
    int ssm = 0;
    uint32_t bits = 0;
    bool normal() const { return ssm == 3; }
    bool valid() const { return ssm == 3 || ssm == 2; }
    bool ncd() const { return ssm == 1; }
    float f32() const {
        float f;
        std::memcpy(&f, &bits, 4);
        return f;
    }
    bool bit(int n) const {
        if (!valid()) return false;
        float f = f32();
        if (!std::isfinite(f)) return false;
        return ((int32_t(int64_t(f)) >> (n - 1)) & 1) != 0;
    }
};

Arinc arinc(double raw) {
    Arinc a;
    double hi = std::floor(raw / 4294967296.0);
    a.ssm = int(hi);
    a.bits = uint32_t(uint64_t(raw - hi * 4294967296.0));
    return a;
}

SkPaint fill(uint32_t c) {
    SkPaint p;
    p.setAntiAlias(true);
    p.setColor(rgb(c));
    return p;
}

std::string signed0(double v) {
    char b[32];
    std::snprintf(b, sizeof b, "%s%.0f", v >= 0 ? "+" : "", v);
    return b;
}

std::string round100(double v) {
    char b[32];
    std::snprintf(b, sizeof b, "%.0f", std::round(v / 100.0) * 100.0);
    return b;
}

struct StsMsg {
    const char* id;
    int color;
    const char* text;
};

const StsMsg kDeferred[] = {
    {"210700001", 0, "WHEN PACK OVHT OUT"},
    {"210700002", 0, "WHEN DIFF PR<2 PSI & FL<100/MEA-MORA"},
    {"221700001", 1, "LDG ELEVN"},
    {"270700001", 2, "RUDDER TRIM RUNAWAY"},
    {"320700001", 3, "L/G GRVTY EXTN"},
    {"340700001", 1, "LDG ELEVN"},
    {"340700003", 1, "LDG ELEVN"},
    {"340700002", 1, "AIR DATA SELECTION"},
};

const StsMsg kInfos[] = {
    {"210200001", 3, "CABIN TEMP REGUL DEGRADED"},
    {"210200002", 3, "FWD CRG VENT DEGRADED"},
    {"210200003", 3, "FWD CRG TEMP REGUL DEGRADED"},
    {"220200001", 3, "FMS 1 ON FMC-C"},
    {"220200002", 3, "FMS 2 ON FMC-C"},
    {"220200003", 3, "STBY INSTRUMENTS NAV AVAIL"},
    {"220200004", 3, "LAND 2 ONLY"},
    {"220200005", 3, "LAND 3 SINGLE ONLY"},
    {"220200006", 3, "FOR AUTOLAND: MAN ROLL OUT ONLY"},
    {"220200007", 3, "AD/FD APPR MODE NOT AVAIL"},
    {"220200008", 3, "LOC MODE AVAIL ONLY"},
    {"220200009", 3, "WHEN L/G DOWN AND AP OFF: USE MAN PITCH TRIM"},
    {"220200010", 3, "APPR 1 ONLY"},
    {"220200011", 3, "FMS PRED UNRELIABLE"},
    {"220200012", 3, "MINIMIZE XWIND FOR LANDING"},
    {"220200013", 3, "AUTOLAND : RECOMMENDED"},
    {"220200014", 3, "FUEL CONSUMPT INCRSD"},
    {"220200015", 3, "RISK OF UNDUE OVERSPEED WARNING"},
    {"230200001", 3, "SATCOM DATALINK AVAIL"},
    {"260200001", 3, "BEFORE CARGO OPENING : PAX DISEMBARK"},
    {"270200001", 3, "ON DRY RWY ONLY : LDG DIST AFFECTED < 15%"},
    {"270200002", 3, "GND SPLRs WILL EXTEND AT REV SELECTION"},
    {"270200003", 3, "F/CTL BKUP CTL ACTIVE"},
    {"270200004", 3, "AUDIOS NOT AVAIL : WINDHSHEAR, SPEED SPEED"},
    {"270200005", 3, "F/CTL INDICATIONS LOST"},
    {"320200001", 3, "ALTN BRK WITH A-SKID"},
    {"320200002", 3, "BRK PRESS AUTO LIMITED ON ALL L/Gs"},
    {"320200003", 3, "DELAY BRAKING UNTIL NLG TOUCHDOWN"},
    {"320200004", 3, "FOR LDG:USE DIFF BRAKING AS RQRD"},
    {"340200002", 3, "ALTN LAW : PROT LOST"},
    {"340200003", 3, "FLS LIMITED TO F-APP + RAW"},
    {"340200004", 3, "DIRECT LAW : PROT LOST"},
    {"340200005", 3, "PFD BKUP SPEED & ALT AVAIL"},
    {"340200006", 3, "FPV / VV AVAIL"},
    {"340200007", 3, "CABIN ALT TRGT: SEE FCOM"},
    {"340200008", 3, "STANDBY NAV IN TRUE GPS TRK"},
    {"340200009", 3, "ADJUST THR TO MAINTAIN ALTITUDE"},
    {"340200010", 3, "FOR CRUISE:"},
    {"340200011", 3, "\xc2\xa0" "\xc2\xa0" "PITCH TRGT : 3°"},
    {"340200012", 3, "\xc2\xa0" "\xc2\xa0" "ALL ENG THR : SEE FCOM"},
    {"340200013", 3, "FOR DESCENT:"},
    {"340200014", 3, "\xc2\xa0" "\xc2\xa0" "PITCH TRGT : 1.25°"},
    {"340200015", 3, "\xc2\xa0" "\xc2\xa0" "ALL THR LEVERS IDLE"},
    {"340200016", 3, "IF RADOME DESTRUCTION :"},
    {"340200017", 3, "\xc2\xa0" "\xc2\xa0" "INCREASE THRUST BY 5 %"},
    {"340200018", 3, "\xc2\xa0" "\xc2\xa0" "FUEL CONSUMPT INCRSD"},
    {"340200019", 3, "\xc2\xa0" "\xc2\xa0" "FMS PRED UNRELIABLE"},
    {"800200002", 3, "ON DRY RWY ONLY : LDG DIST AFFECTED < 15%"},
    {"800200003", 3, "TAXI WITH CARE"},
    {"800200004", 5, "AVOID MAX TILLER ANGLE TURN ON WET/CONTAM RWY"},
    {"800200005", 3, "NO BRAKED PIVOT TURN"},
};
const StsMsg kLimits[] = {
    {"1", 2, "LAND ASAP"},
    {"2", 4, "LAND ANSA"},
    {"210400001", 5, "MAX FL : 100/MEA-MORA"},
    {"210400003", 5, "MAN PRESS : DO NOT USE"},
    {"210400004", 5, "AVOID HI DESCENT V/S"},
    {"220400001", 5, "NO AUTOLAND"},
    {"230400001", 5, "NO COM AVAIL"},
    {"240400001", 5, "GA THR : TOGA ONLY"},
    {"240400002", 5, "MAX SPEED: 310/.86"},
    {"240400003", 5, "SPD BRK: DO NOT USE"},
    {"240400004", 5, "MANEUVER WITH CARE"},
    {"260400001", 5, "APU BLEED DO NOT USE"},
    {"260400002", 5, "MAX SPEED : 250/.55"},
    {"270400001", 5, "FOR LDG : FLAP LVR 3"},
    {"270400002", 5, "MAX SPEED: 310 KT"},
    {"270400003", 5, "USE RUDDER WITH CARE"},
    {"270400004", 5, "FOR LDG : FLAP LVR 1"},
    {"290400001", 5, "SLATS SLOW"},
    {"290400002", 5, "FLAPS SLOW"},
    {"300400001", 5, "AVOID ICING CONDs"},
    {"320400001", 5, "MAX SPEED : 220 KT"},
    {"320400002", 5, "L/G GRVTY EXTN ONLY"},
    {"320400003", 5, "STEER ENDUR LIMITED"},
    {"320400004", 5, "AUTO BRK:DO NOT USE"},
    {"340400001", 5, "ALL SPEED : MAX 330 / M .82"},
    {"700400001", 5, "REV : SYM USE ONLY"},
    {"800400001", 5, "FUEL CONSUMPT INCRSD"},
    {"800400002", 5, "LDG DIST AFFECTED"},
    {"800400003", 5, "LDG PERF AFFECTED"},
    {"800400004", 5, "FOR GA : KEEP S/F CONF"},
    {"800400005", 5, "MAX DIFF PR : 1 PSI"},
    {"800400006", 5, "MAX SPEED : 300/.85"},
    {"800400007", 5, "MIN SPEED : GREEN DOT"},
    {"800400008", 5, "TURB SPEED:"},
};
const StsMsg kInop[] = {
    {"210300001", 4, "PACK 1 CTL 1"},
    {"210300002", 4, "PACK 1 CTL 2"},
    {"210300003", 4, "PACK 2 CTL 1"},
    {"210300004", 4, "PACK 2 CTL 2"},
    {"210300005", 4, "PACK 1 CTL DEGRADED"},
    {"210300006", 4, "PACK 2 CTL DEGRADED"},
    {"210300007", 4, "PACK 1 CTL REDUND"},
    {"210300008", 4, "PACK 2 CTL REDUND"},
    {"210300009", 4, "PACK 1"},
    {"210300010", 4, "PACK 2"},
    {"210300011", 4, "PACK 1+2"},
    {"210300012", 4, "ALL PRIMARY CAB FANS"},
    {"210300013", 4, "BULK CRG HEATER"},
    {"210300014", 4, "BULK CRG ISOL"},
    {"210300015", 4, "BULK CRG VENT"},
    {"210300016", 4, "FWD CRG ISOL"},
    {"210300017", 4, "FWD CRG VENT"},
    {"210300018", 4, "FWD CRG TEMP REGUL"},
    {"210300019", 4, "CKPT TEMP REGUL"},
    {"210300020", 4, "CAB PART TEMP REGUL"},
    {"210300021", 4, "TEMP CTL 1"},
    {"210300022", 4, "TEMP CTL 2"},
    {"210300023", 4, "ALL ZONES TEMP REGUL"},
    {"210300024", 4, "TEMP CTL REDUND"},
    {"210300025", 4, "1 PRIMARY CAB FAN"},
    {"210300026", 4, "2 PRIMARY CAB FANS"},
    {"210300027", 4, "3 PRIMARY CAB FANS"},
    {"210300028", 4, "TEMP CTL DEGRADED"},
    {"212300001", 4, "AFT VENT CTL 1"},
    {"212300002", 4, "AFT VENT CTL 2"},
    {"212300003", 4, "AFT VENT CTL DEGRADED"},
    {"212300004", 4, "CAB AIR EXTRACT VLV"},
    {"212300005", 4, "AFT VENT CTL"},
    {"212300006", 4, "AFT VENT CTL REDUND"},
    {"212300007", 4, "FWD VENT CTL 1"},
    {"212300008", 4, "FWD VENT CTL 2"},
    {"212300009", 4, "FWD VENT CTL DEGRADED"},
    {"212300010", 4, "IFE BAY VENT"},
    {"212300011", 4, "RAM AIR"},
    {"212300012", 4, "FWD VENT CTL"},
    {"212300013", 4, "FWD VENT CTL REDUND"},
    {"213300001", 4, "OUTFLW VLV CTL 1 FAULT"},
    {"213300002", 4, "OUTFLW VLV CTL 2 FAULT"},
    {"213300003", 4, "OUTFLW VLV CTL 3 FAULT"},
    {"213300004", 4, "OUTFLW VLV CTL 4 FAULT"},
    {"213300005", 4, "CAB PRESS SYS"},
    {"213300006", 4, "OUTFLW VLV CTL 1+2"},
    {"213300007", 4, "OUTFLW VLV CTL 1+3"},
    {"213300008", 4, "OUTFLW VLV CTL 1+4"},
    {"213300009", 4, "OUTFLW VLV CTL 2+3"},
    {"213300010", 4, "OUTFLW VLV CTL 2+4"},
    {"213300011", 4, "OUTFLW VLV CTL 1+2+3"},
    {"213300012", 4, "OUTFLW VLV CTL 1+2+4"},
    {"213300013", 4, "OUTFLW VLV CTL 1+3+4"},
    {"213300014", 4, "OUTFLW VLV CTL 2+3+4"},
    {"213300015", 4, "OUTFLW VLV CTL 2+4"},
    {"213300016", 4, "CAB PRESS AUTO CTL 1"},
    {"213300017", 4, "CAB PRESS AUTO CTL 2"},
    {"213300018", 4, "CAB PRESS AUTO CTL 3"},
    {"213300019", 4, "CAB PRESS AUTO CTL 4"},
    {"220300001", 4, "A/THR"},
    {"220300004", 4, "AFS CTL PNL"},
    {"220300005", 4, "AP 1"},
    {"220300006", 4, "AP 2"},
    {"220300007", 4, "AP 1+2"},
    {"220300010", 4, "GLS AUTOLAND"},
    {"220300012", 4, "CAPT AFS BKUP CTL"},
    {"220300013", 4, "F/O AFS BKUP CTL"},
    {"220300014", 4, "ENG 1 A/THR"},
    {"220300015", 4, "ENG 2 A/THR"},
    {"220300016", 4, "ENG 3 A/THR"},
    {"220300017", 4, "ENG 4 A/THR"},
    {"220300018", 4, "ROLL OUT"},
    {"220300020", 4, "AP/FD TCAS MODE"},
    {"220300021", 4, "REACTIVE W/S DET"},
    {"220300022", 4, "FD 1"},
    {"220300023", 4, "FD 2"},
    {"220300024", 4, "FD 1+2"},
    {"220300025", 4, "GA SOFT"},
    {"220300026", 4, "AUTOLAND"},
    {"221300001", 4, "FMC-A"},
    {"221300002", 4, "FMC-B"},
    {"221300003", 4, "FMC-C"},
    {"221300004", 4, "FMS 1"},
    {"221300005", 4, "FMS 2"},
    {"221300006", 4, "FMS 1+2"},
    {"230300001", 4, "CIDS 1+2+3"},
    {"230300002", 4, "UPPER DECK PA"},
    {"230300003", 4, "MAIN DECK PA"},
    {"230300004", 4, "LOWER DECK PA"},
    {"230300005", 4, "CABIN INTERPHONE"},
    {"230300006", 4, "DATALINK"},
    {"230300007", 4, "HF 1 DATALINK"},
    {"230300008", 4, "HF 2 DATALINK"},
    {"230300009", 4, "RMP 1"},
    {"230300010", 4, "RMP 2"},
    {"230300011", 4, "RMP 3"},
    {"230300012", 4, "RMP 1+2"},
    {"230300013", 4, "RMP 1+3"},
    {"230300014", 4, "RMP 2+3"},
    {"230300015", 4, "STBY RAD NAV"},
    {"230300016", 4, "RMP 1+2+3"},
    {"230300017", 4, "VHF 1+2+3"},
    {"230300018", 4, "HF 1+2"},
    {"230300019", 4, "SATCOM"},
    {"230300020", 4, "SATCOM DATALINK"},
    {"230300021", 4, "VHF 3 DATALINK"},
    {"240300001", 4, "APU BAT"},
    {"240300002", 4, "APU GEN A"},
    {"240300003", 4, "APU GEN B"},
    {"240300004", 4, "APU TR"},
    {"240300005", 4, "BAT 1"},
    {"240300006", 4, "BAT 2"},
    {"240300007", 4, "BAT ESS"},
    {"240300008", 4, "BAT 1+2"},
    {"240300009", 4, "C/B MONITORING"},
    {"240300010", 4, "GEN 1"},
    {"240300011", 4, "GEN 2"},
    {"240300012", 4, "GEN 3"},
    {"240300013", 4, "GEN 4"},
    {"240300014", 4, "APU GEN A"},
    {"240300015", 4, "APU GEN B"},
    {"240300016", 4, "EXT PWR 1"},
    {"240300017", 4, "EXT PWR 2"},
    {"240300018", 4, "EXT PWR 3"},
    {"240300019", 4, "EXT PWR 4"},
    {"240300020", 4, "ENMU 1"},
    {"240300021", 4, "ENMU 2"},
    {"240300022", 4, "PART GALLEY"},
    {"240300023", 4, "EMER C/B MONITORING"},
    {"240300024", 4, "ELEC LOAD MANAGT"},
    {"240300025", 4, "ELEC PRIMARY CTR 1"},
    {"240300026", 4, "ELEC PRIMARY CTR 2"},
    {"240300027", 4, "COMMERCIAL"},
    {"240300028", 4, "PART COMMERCIAL"},
    {"240300029", 4, "RAT"},
    {"240300030", 4, "ELEC SECONDARY CTR 1"},
    {"240300031", 4, "ELEC SECONDARY CTR 2"},
    {"240300032", 4, "PART ELEC SEC CTR 1"},
    {"240300033", 4, "PART ELEC SEC CTR 2"},
    {"240300034", 4, "TR 1"},
    {"240300035", 4, "TR 2"},
    {"240300036", 4, "TR ESS"},
    {"240300037", 4, "GEN 1+2"},
    {"240300038", 4, "GEN 3+4"},
    {"260300001", 4, "APU FIRE DET"},
    {"260300002", 4, "ENG 1 BLEED"},
    {"260300003", 4, "ENG 2 BLEED"},
    {"260300004", 4, "ENG 3 BLEED"},
    {"260300005", 4, "ENG 4 BLEED"},
    {"260300006", 4, "APU BLEED"},
    {"260300007", 4, "L X BLEED AUTO CTL"},
    {"260300008", 4, "R X BLEED AUTO CTL"},
    {"260300009", 4, "CTR X BLEED AUTO CTL"},
    {"260300010", 4, "ENG 1 FIRE DET"},
    {"260300011", 4, "ENG 2 FIRE DET"},
    {"260300012", 4, "ENG 3 FIRE DET"},
    {"260300013", 4, "ENG 4 FIRE DET"},
    {"260300014", 4, "ENG 1 FIRE LOOP A"},
    {"260300015", 4, "ENG 1 FIRE LOOP B"},
    {"260300016", 4, "ENG 2 FIRE LOOP A"},
    {"260300017", 4, "ENG 2 FIRE LOOP B"},
    {"260300018", 4, "ENG 3 FIRE LOOP A"},
    {"260300019", 4, "ENG 3 FIRE LOOP B"},
    {"260300020", 4, "ENG 4 FIRE LOOP A"},
    {"260300021", 4, "ENG 4 FIRE LOOP B"},
    {"260300022", 4, "MLG BAY FIRE DET"},
    {"260300023", 4, "MLG BAY FIRE LOOP A"},
    {"260300024", 4, "MLG BAY FIRE LOOP B"},
    {"260300025", 4, "APU FIRE LOOP A"},
    {"260300026", 4, "APU FIRE LOOP B"},
    {"260300027", 4, "AFT AVNCS SMK DET"},
    {"260300028", 4, "ALL CAB FANS"},
    {"260300029", 4, "ALL SMOKE DET"},
    {"260300030", 4, "FWD CRG VENT"},
    {"260300031", 4, "FWD CGR TEMP REGUL"},
    {"260300032", 4, "BULK CRG VENT"},
    {"260300033", 4, "BULK CRG HEATER"},
    {"260300034", 4, "IFE BAY SMK DET"},
    {"260300035", 4, "L MAIN AVNCS SMK DET"},
    {"260300036", 4, "L UPPR AVNCS SMK DET"},
    {"260300037", 4, "R MAIN AVNCS SMK DET"},
    {"260300038", 4, "R UPPR AVNCS SMK DET"},
    {"260300039", 4, "FACILITIES SMK DET"},
    {"260300040", 4, "FWD CARGO BTLs"},
    {"260300041", 4, "AFT CARGO BTLs"},
    {"260300042", 4, "FWD CRG TEMP REGUL"},
    {"260300043", 4, "FWD CARGO SMK DET"},
    {"260300044", 4, "AFT CARGO SMK DET"},
    {"260300045", 4, "FWD+AFT CRGs BTL 1"},
    {"260300046", 4, "FWD+AFT CRGs BTL 2"},
    {"260300047", 4, "FWD LWR REST BTL 1"},
    {"260300048", 4, "FWD LWR REST BTL 2"},
    {"260300049", 4, "FWD LWR REST SMK DET"},
    {"260300050", 4, "M5L FLT REST SMK DET"},
    {"260300051", 4, "M5L CAB REST SMK DET"},
    {"260300052", 4, "MAIN LAV SMK DET"},
    {"260300053", 4, "UPPER LAV SMK DET"},
    {"260300054", 4, "LOWER LAV SMK DET"},
    {"260300055", 4, "M2L CWS SMK DET"},
    {"260300056", 4, "M2L RCC SMK DET"},
    {"260300057", 4, "U1L CWS SMK DET"},
    {"260300058", 4, "U1L RCC SMK DET"},
    {"260300059", 4, "U3R CWS SMK DET"},
    {"260300060", 4, "U3R RCC SMK DET"},
    {"260300061", 4, "U1L SHOWER SMK DET"},
    {"260300062", 4, "U1R SHOWER SMK DET"},
    {"270300001", 4, "SEC 1"},
    {"270300002", 4, "SEC 2"},
    {"270300003", 4, "SEC 3"},
    {"270300004", 4, "RUDDER TRIM"},
    {"270300005", 4, "RUDDER TRIM 1"},
    {"270300006", 4, "RUDDER TRIM 2"},
    {"270300007", 4, "F/CTL REDUNDANCY"},
    {"270300008", 4, "UPPR RUDDER"},
    {"270300009", 4, "LWR RUDDER"},
    {"270300010", 4, "PRIM 1"},
    {"270300011", 4, "PRIM 2"},
    {"270300012", 4, "PRIM 3"},
    {"270300013", 4, "FCDC 1"},
    {"270300014", 4, "FCDC 2"},
    {"270300015", 4, "FCDC 1+2"},
    {"290100001", 4, "PART SPLRs"},
    {"290100003", 4, "FLAP SYS 1"},
    {"290100004", 4, "FLAP SYS 2"},
    {"290100005", 4, "SLAT SYS 1"},
    {"290100006", 4, "SLAT SYS 2"},
    {"290100007", 4, "STABILIZER"},
    {"290100008", 4, "F/CTL PROT"},
    {"290100009", 4, "L OUTR AILERON"},
    {"290100010", 4, "R OUTR AILERON"},
    {"290100011", 4, "MOST SPLRs"},
    {"290100012", 4, "FLAPS"},
    {"290100013", 4, "SLATS"},
    {"290100014", 4, "F/CTL ALTN LAW"},
    {"290300001", 4, "G ELEC PMP A"},
    {"290300002", 4, "G ELEC PMP B"},
    {"290300003", 4, "Y ELEC PMP A"},
    {"290300004", 4, "Y ELEC PMP B"},
    {"290300005", 4, "G ENG 1 PMP A"},
    {"290300006", 4, "G ENG 1 PMP B"},
    {"290300007", 4, "G ENG 2 PMP A"},
    {"290300008", 4, "G ENG 2 PMP B"},
    {"290300009", 4, "Y ENG 3 PMP A"},
    {"290300010", 4, "Y ENG 3 PMP B"},
    {"290300011", 4, "Y ENG 4 PMP A"},
    {"290300012", 4, "Y ENG 4 PMP B"},
    {"290300013", 4, "G SYS CHAN A OVHT DET"},
    {"290300014", 4, "G SYS CHAN B OVHT DET"},
    {"290300015", 4, "Y SYS CHAN A OVHT DET"},
    {"290300016", 4, "Y SYS CHAN B OVHT DET"},
    {"290300017", 4, "G HSMU"},
    {"290300018", 4, "Y HSMU"},
    {"290300019", 4, "G SYS OVHT DET"},
    {"290300020", 4, "Y SYS OVHT DET"},
    {"290300021", 4, "G HYD SYS"},
    {"290300022", 4, "Y HYD SYS"},
    {"290300023", 4, "G ENG 1 PMP A+B"},
    {"290300024", 4, "G ENG 2 PMP A+B"},
    {"290300025", 4, "Y ENG 3 PMP A+B"},
    {"290300026", 4, "Y ENG 4 PMP A+B"},
    {"310300001", 4, "AUTO CALLOUT"},
    {"310300002", 4, "FWS 1"},
    {"310300003", 4, "FWS 2"},
    {"320300001", 4, "A-SKID"},
    {"320300002", 4, "AUTO BRK"},
    {"320300003", 4, "PART A-SKID"},
    {"320300004", 4, "BRK ACCU"},
    {"320300005", 4, "ALTN BRK"},
    {"320300006", 4, "EMER BRK"},
    {"320300007", 4, "BTV"},
    {"320300008", 4, "NORM BRK"},
    {"320300009", 4, "PARK BRK"},
    {"320300010", 4, "PEDAL BRAKING"},
    {"320300011", 4, "L/G CTL 1+2"},
    {"320300012", 4, "L/G DOORS"},
    {"320300013", 4, "L/G RETRACTION"},
    {"320300014", 4, "B/W STEER"},
    {"320300015", 4, "CAPT STEER TILLER"},
    {"320300016", 4, "FO STEER TILLER"},
    {"320300017", 4, "N/W + B/W STEER"},
    {"320300018", 4, "N/W STEER DISC"},
    {"320300019", 4, "N/W STEER"},
    {"320300020", 4, "NORM N/W STEER"},
    {"320300021", 4, "PEDAL STEER CTL"},
    {"320300022", 4, "ROW/ROP"},
    {"320300023", 4, "PART L/G RETRACTION"},
    {"320300024", 4, "NORM B/W STEER"},
    {"320300025", 4, "RWY OVERRUN PROT"},
    {"340300001", 4, "GPWS 1"},
    {"340300002", 4, "GPWS 2"},
    {"340300003", 4, "GPWS 1+2"},
    {"340300004", 4, "ADR 1"},
    {"340300005", 4, "ADR 2"},
    {"340300006", 4, "ADR 3"},
    {"340300007", 4, "ADR 1+2"},
    {"340300008", 4, "ADR 2+3"},
    {"340300009", 4, "ADR 1+3"},
    {"340300010", 4, "ADR 1+2+3"},
    {"340300011", 4, "TCAS 1"},
    {"340300012", 4, "TCAS 2"},
    {"340300014", 4, "LOAD ALLEVIATION"},
    {"340300021", 4, "GUST LOAD PROT"},
    {"340300022", 4, "RA SYS A"},
    {"340300023", 4, "RA SYS B"},
    {"340300024", 4, "RA SYS C"},
    {"340300025", 4, "RA SYS A+B"},
    {"340300026", 4, "RA SYS A+C"},
    {"340300027", 4, "RA SYS B+C"},
    {"340300028", 4, "RA SYS A+B+C"},
    {"340300029", 4, "TCAS 1+2"},
    {"340300030", 4, "IR 1"},
    {"340300031", 4, "IR 2"},
    {"340300032", 4, "IR 3"},
    {"340300033", 4, "IR 1+2"},
    {"340300034", 4, "IR 1+3"},
    {"340300035", 4, "IR 2+3"},
    {"340300036", 4, "IR 1+2+3"},
    {"340300037", 4, "WXR 1"},
    {"340300038", 4, "WXR 2"},
    {"340300043", 4, "WXR 1+2"},
    {"340300039", 4, "TERR SYS 1"},
    {"340300040", 4, "TERR SYS 2"},
    {"340300044", 4, "TERR SYS 1+2"},
    {"340300041", 4, "ADS-B RPTG 1"},
    {"340300042", 4, "ADS-B RPTG 2"},
    {"340300045", 4, "ADS-B RPTG 1+2"},
    {"341300001", 4, "PRED W/S 1"},
    {"341300002", 4, "PRED W/S 2"},
    {"341300003", 4, "PRED W/S 1+2"},
    {"340300046", 4, "TAWS SYS 1"},
    {"340300047", 4, "TAWS SYS 2"},
    {"340300048", 4, "TAWS SYS 1+2"},
    {"340300049", 4, "ARPT NAV"},
    {"700300001", 4, "ENG 2 REVERSER"},
    {"700300002", 4, "ENG 3 REVERSER"},
    {"700300003", 4, "ENG 2+3 REVERSERs"},
};

const uint8_t* mask_lut(uint32_t color) {
    thread_local std::vector<uint8_t> cache;
    const float contrast = 1.0f, gamma = 2.2f;
    if (cache.empty()) {
        cache.assign(8 * 256, 0);
        auto to_lin = [&](float v) { return std::pow(v, gamma); };
        auto from_lin = [&](float v) { return std::pow(v, 1.f / gamma); };
        for (int k = 0; k < 8; k++) {
            int b = k << 5, lum = b;
            for (int q = 3; q < 8; q += 3) lum |= b >> q;
            float src = lum / 255.f, dst = 1.f - src, ls = to_lin(src), ld = to_lin(dst), adj = contrast * ld;
            for (int i = 0; i < 256; i++) {
                float raw = i / 255.f, sa = raw + (1.f - raw) * adj * raw;
                float out = from_lin(ls * sa + (1.f - sa) * ld);
                float r = (out - dst) / (src - dst);
                cache[(size_t)k * 256 + i] = (uint8_t)std::min(255L, std::max(0L, std::lround(255.f * r)));
            }
        }
    }
    int lum = (((color >> 16) & 255) * 54 + ((color >> 8) & 255) * 183 + (color & 255) * 19) >> 8;
    return &cache[(size_t)(lum >> 5) * 256];
}

void stroked_mask(SkCanvas& cv, const std::vector<SkPath>& paths, const SkPaint& paint) {
    if (paths.empty()) return;
    SkMatrix m = cv.getTotalMatrix();
    SkRect b = SkRect::MakeEmpty();
    for (auto& pa : paths) b.join(pa.getBounds());
    SkRect db = m.mapRect(b);
    int x0 = (int)std::floor(db.fLeft) - 4, y0 = (int)std::floor(db.fTop) - 4;
    int w = (int)std::ceil(db.fRight) + 4 - x0, h = (int)std::ceil(db.fBottom) + 4 - y0;
    SkBitmap bm;
    bm.allocPixels(SkImageInfo::MakeA8(w, h));
    bm.eraseColor(0);
    SkCanvas mc(bm);
    mc.translate(-(float)x0, -(float)y0);
    mc.concat(m);
    SkPaint wp(paint);
    wp.setColor(SK_ColorWHITE);
    for (auto& pa : paths) mc.drawPath(pa, wp);
    const uint8_t* lut = mask_lut(paint.getColor() & 0xffffff);
    for (int j = 0; j < h; j++) {
        uint8_t* d = bm.getAddr8(0, j);
        for (int i = 0; i < w; i++) d[i] = lut[d[i]];
    }
    cv.save();
    cv.resetMatrix();
    SkPaint ip;
    ip.setColor(paint.getColor());
    cv.drawImage(bm.asImage(), (float)x0, (float)y0, SkSamplingOptions(), &ip);
    cv.restore();
}

sk_sp<SkImage> load_image(const char* name) {
    for (auto& dir : a380::assets::images_dirs()) {
        auto data = SkData::MakeFromFileName((dir + "/" + name).c_str());
        if (!data) continue;
        auto codec = SkCodec::MakeFromData(data);
        if (!codec) continue;
        auto [image, result] = codec->getImage();
        if (image) return image;
    }
    return nullptr;
}

void draw_image(SkCanvas& cv, const char* name, float x, float y, float w, float h) {
    thread_local std::vector<std::pair<std::string, sk_sp<SkImage>>> cache;
    sk_sp<SkImage> img;
    for (auto& c : cache)
        if (c.first == name) img = c.second;
    if (!img) {
        img = load_image(name);
        cache.emplace_back(name, img);
    }
    if (img) cv.drawImageRect(img, SkRect::MakeXYWH(x, y, w, h), SkSamplingOptions(SkFilterMode::kLinear, SkMipmapMode::kLinear));
}

struct Sd : a380::Display {
    SkISize size() const override { return {768, 1024}; }

    static void text(SkCanvas& cv, const std::string& s, float x, float y, float px, uint32_t c,
                     Align al = Align::Left, float ls = 0.f) {
        SkFont f = a380::font("Ecam", px);
        SkPaint p = fill(c);
        if (c == kGreen || c == kAmber) {
            p.setStyle(SkPaint::kStrokeAndFill_Style);
            std::vector<SkGlyphID> g(s.size());
            size_t n = f.textToGlyphs(s.data(), s.size(), SkTextEncoding::kUTF8, SkSpan<SkGlyphID>(g));
            g.resize(n);
            std::vector<SkScalar> wd(n);
            f.getWidths(SkSpan<const SkGlyphID>(g), SkSpan<SkScalar>(wd));
            float tw = 0;
            for (float v : wd) tw += v + ls;
            float cx = al == Align::Left ? x : al == Align::Center ? x - tw * 0.5f : x - tw;
            y = a380::chromium_snap(y, 1.5f);
            if (sdsys::stroked_text_fir(cv, s, cx, y, f, c, 1.08f, ls)) return;
            p.setStrokeWidth(1.08f);
            std::vector<SkPath> paths;
            for (size_t i = 0; i < n; i++) {
                if (auto path = f.getPath(g[i])) paths.push_back(path->makeTransform(SkMatrix::Translate(std::floor(cx * 6.f + 0.5f) / 6.f, y)));
                cx += wd[i] + ls;
            }
            stroked_mask(cv, paths, p);
            return;
        }
        a380::draw_text(cv, s, x, y, f, p, al, a380::Baseline::Alphabetic, ls);
    }

    static void rule(SkCanvas& cv, float x1, float y1, float x2, float y2) {
        SkPaint p = fill(kWhite);
        p.setStyle(SkPaint::kStroke_Style);
        p.setStrokeWidth(4.f);
        p.setStrokeCap(SkPaint::kRound_Cap);
        cv.drawLine(x1, y1, x2, y2, p);
    }

    void permanent(SkCanvas& cv, const a380::Inputs& in) {
        const std::string air = in.var("L:A32NX_AIR_DATA_SWITCHING_KNOB") == 0 ? "3" : "1";
        const std::string att = in.var("L:A32NX_ATT_HDG_SWITCHING_KNOB") == 0 ? "3" : "1";
        Arinc sat = arinc(in.var("L:A32NX_ADIRS_ADR_" + air + "_STATIC_AIR_TEMPERATURE"));
        Arinc tat = arinc(in.var("L:A32NX_ADIRS_ADR_" + air + "_TOTAL_AIR_TEMPERATURE"));
        Arinc zp = arinc(in.var("L:A32NX_ADIRS_ADR_" + air + "_ALTITUDE"));
        Arinc fl = arinc(in.var("L:A32NX_FCU_EFIS_L_DISCRETE_WORD_2"));
        Arinc fr = arinc(in.var("L:A32NX_FCU_EFIS_R_DISCRETE_WORD_2"));
        Arinc acc = arinc(in.var("L:A32NX_ADIRS_IR_" + att + "_BODY_NORMAL_ACC"));
        Arinc gw = arinc(in.var("L:A32NX_FQMS_GROSS_WEIGHT"));
        Arinc cg = arinc(in.var("L:A32NX_FQMS_CENTER_OF_GRAVITY_MAC"));
        Arinc fob = arinc(in.var("L:A32NX_FQMS_TOTAL_FUEL_ON_BOARD"));
        double zulu = in.var("E:ZULU TIME", {}, 0.0);

        rule(cv, 7, 667, 761, 667);
        {
            SkPaint p = fill(kWhite);
            p.setStyle(SkPaint::kStroke_Style);
            p.setStrokeWidth(4.f);
            cv.drawLine(0, 765, 768, 765, p);
        }
        rule(cv, 257, 667, 257, 759);
        rule(cv, 512, 667, 512, 759);

        text(cv, "TAT", 34, 696, 26, kWhite, Align::Left, 1.f);
        text(cv, tat.normal() ? signed0(tat.f32()) : "XX", 158, 696, 25, tat.normal() ? kGreen : kAmber, Align::Right);
        text(cv, "\xC2\xB0" "C", 185, 696, 26, kCyan);
        text(cv, "SAT", 34, 725, 26, kWhite, Align::Left, 1.f);
        text(cv, sat.normal() ? signed0(sat.f32()) : "XX", 158, 725, 25, sat.normal() ? kGreen : kAmber, Align::Right);
        text(cv, "\xC2\xB0" "C", 185, 725, 26, kCyan);

        bool fcuOk = !fl.valid() || fl.bit(11) || !fr.valid() || fr.bit(11);
        if (fcuOk && zp.normal() && sat.normal()) {
            double isa = sat.f32() + std::min(36089.f, zp.f32()) / 500.0 - 15.0;
            text(cv, "ISA", 34, 754, 26, kWhite, Align::Left, 1.f);
            text(cv, signed0(isa), 158, 754, 25, kGreen, Align::Right);
            text(cv, "\xC2\xB0" "C", 185, 754, 26, kCyan);
        }

        if (acc.normal() && (acc.f32() < 0.7f || acc.f32() > 1.4f)) {
            char b[32];
            std::snprintf(b, sizeof b, "%s%.1f", acc.f32() >= 0 ? "+" : "", acc.f32());
            text(cv, "G LOAD", 296, 702, 27, kAmber);
            text(cv, b, 410, 702, 27, kAmber);
        }

        int t = int(std::floor(zulu));
        int h = (t / 3600) % 24, m = (t / 60) % 60, s = t % 60;
        char hm[16], ss[8];
        std::snprintf(hm, sizeof hm, "%02d:%02d:", h, m);
        std::snprintf(ss, sizeof ss, "%02d", s);
        text(cv, hm, 296, 730, 29, kGreen, Align::Left, -1.f);
        text(cv, ss, 394, 730, 26, kGreen);
        text(cv, "GPS", 434, 729, 22, kGreen);

        text(cv, "GW", 529, 696, 25, kWhite);
        text(cv, "GWCG", 529, 724, 25, kWhite);
        text(cv, "FOB", 529, 752, 25, kWhite);

        auto w = [&](const Arinc& a, const std::string& ok, float y, bool showNcd) {
            bool dash = showNcd && a.ncd();
            std::string s2 = a.normal() ? ok : dash ? "--\xC2\xA0\xC2\xA0" : "XX\xC2\xA0\xC2\xA0";
            uint32_t c = a.normal() ? kGreen : dash ? kCyan : kAmber;
            text(cv, s2, 705, y, 27, c, Align::Right);
        };
        char cgs[16];
        std::snprintf(cgs, sizeof cgs, "%.1f", cg.f32());
        w(gw, round100(gw.f32()), 696, true);
        w(cg, cgs, 724, true);
        w(fob, round100(fob.f32()), 752, false);
        text(cv, "KG", 711, 696, 22, kCyan);
        text(cv, "%", 711, 724, 22, kCyan);
        text(cv, "KG", 711, 752, 22, kCyan);
    }

    static void poly(SkCanvas& cv, std::initializer_list<SkPoint> pts, uint32_t c) {
        SkPathBuilder b;
        bool first = true;
        for (auto& q : pts) {
            if (first) b.moveTo(q.fX, q.fY);
            else b.lineTo(q.fX, q.fY);
            first = false;
        }
        b.close();
        a380::draw_path_cached(cv, b.detach(), fill(c));
    }

    static SkRect device_snap(SkCanvas& cv, const SkRect& r) {
        SkRect d = cv.getTotalMatrix().mapRect(r);
        return SkRect::MakeLTRB(std::round(d.fLeft), std::round(d.fTop), std::round(d.fRight), std::round(d.fBottom));
    }

    static void snapped_rect(SkCanvas& cv, const SkRect& r, uint32_t c) {
        SkRect d = device_snap(cv, r);
        cv.save();
        cv.resetMatrix();
        SkPaint p = fill(c);
        p.setAntiAlias(false);
        cv.drawRect(d, p);
        cv.restore();
    }

    static void outset_box(SkCanvas& cv, const SkRect& css, float border_css) {
        SkMatrix m = cv.getTotalMatrix();
        SkRect r = SkRect::MakeLTRB(std::floor(m.mapRect(css).fLeft + 0.5f), std::floor(m.mapRect(css).fTop + 0.5f),
                                    std::floor(m.mapRect(css).fRight + 0.5f), std::floor(m.mapRect(css).fBottom + 0.5f));
        float b = std::max(1.f, std::floor(border_css * m.getScaleX()));
        cv.save();
        cv.resetMatrix();
        SkPaint dark = fill(0xababab);
        dark.setAntiAlias(false);
        cv.drawRect(r, dark);
        SkPaint inner = fill(0x000000);
        inner.setAntiAlias(false);
        cv.drawRect(r.makeInset(b, b), inner);
        SkPaint lit = fill(kWhite);
        lit.setAntiAlias(true);
        SkPathBuilder pb;
        pb.moveTo(r.fLeft, r.fTop);
        pb.lineTo(r.fRight, r.fTop);
        pb.lineTo(r.fRight - b, r.fTop + b);
        pb.lineTo(r.fLeft + b, r.fTop + b);
        pb.lineTo(r.fLeft + b, r.fBottom - b);
        pb.lineTo(r.fLeft, r.fBottom);
        pb.close();
        a380::draw_path_cached(cv, pb.detach(), lit);
        cv.restore();
    }

    static void button_box(SkCanvas& cv, float x, float y, float w, float h) {
        SkRect d = device_snap(cv, SkRect::MakeXYWH(x, y, w, h));
        float bw = std::max(1.f, std::round(cv.getTotalMatrix().mapRadius(2.f)));
        cv.save();
        cv.resetMatrix();
        float l = d.fLeft, t = d.fTop, r = d.fRight, b = d.fBottom;
        cv.drawRect(d, fill(0x272525));
        poly(cv, {{l, t}, {r, t}, {r - bw, t + bw}, {l + bw, t + bw}}, 0xf2f4ff);
        poly(cv, {{l, t}, {l + bw, t + bw}, {l + bw, b - bw}, {l, b}}, 0xf2f4ff);
        poly(cv, {{l, b}, {l + bw, b - bw}, {r - bw, b - bw}, {r, b}}, 0x74757b);
        poly(cv, {{r, t}, {r, b}, {r - bw, b - bw}, {r - bw, t + bw}}, 0x74757b);
        cv.restore();
    }

    static void button(SkCanvas& cv, const std::string& l1, const std::string& l2, float x, float y, float w,
                       bool right) {
        button_box(cv, x, y, w, 50.f);
        SkFont f = a380::font("FBW-Display-EIS-A380-SlashedZero", 22.f);
        const float lh = 19.3333f, mid = y + 27.f, right_edge = x + w - 13.93f;
        Align al = right ? Align::Right : Align::Center;
        float tx = right ? right_edge : x + w * 0.5f;
        if (l2.empty()) {
            float base = a380::chromium_baseline(f, a380::chromium_snap(mid - lh * 0.5f, 1.5f), lh, 1.5f);
            a380::draw_text(cv, l1, tx, base, f, fill(kWhite), al, a380::Baseline::Alphabetic);
        } else {
            float w1 = a380::text_width(f, l1), w2 = a380::text_width(f, l2), bw = std::max(w1, w2);
            float cx = right ? x + 14.07f + (w - 28.f - bw) * 0.5f : tx - bw * 0.5f;
            a380::draw_text(cv, l1, cx, a380::chromium_baseline(f, a380::chromium_snap(mid - lh, 1.5f), lh, 1.5f), f, fill(kWhite), Align::Left, a380::Baseline::Alphabetic);
            a380::draw_text(cv, l2, cx, a380::chromium_baseline(f, a380::chromium_snap(mid, 1.5f), lh, 1.5f), f, fill(kWhite), Align::Left, a380::Baseline::Alphabetic);
        }
    }
    static void icon_button(SkCanvas& cv, float x, float y, bool up) {
        button_box(cv, x, y, 70.f, 40.f);
        float ox = x + 35.f - 17.5f, oy = y + 20.f - 17.5f;
        cv.save();
        cv.translate(ox, oy);
        if (!up) {
            cv.translate(17.5f, 17.5f);
            cv.rotate(180.f);
            cv.translate(-17.5f, -17.5f);
        }
        poly(cv, {{0, 17.5f}, {17.5f, 0}, {35, 17.5f}}, kWhite);
        poly(cv, {{0, 35}, {17.5f, 17.5f}, {35, 35}}, kWhite);
        cv.restore();
    }

    void mailbox(SkCanvas& cv) {
        snapped_rect(cv, SkRect::MakeXYWH(97.84f, 768.f, 2.f, 256.f), kWhite);
        snapped_rect(cv, SkRect::MakeXYWH(629.76f, 768.f, 2.f, 256.f), kWhite);
        snapped_rect(cv, SkRect::MakeXYWH(99.84f, 768.f + 226.f, 529.92f, 2.f), kWhite);
        snapped_rect(cv, SkRect::MakeXYWH(99.84f + 264.96f - 1.f, 768.f + 228.f, 2.f, 28.f), kWhite);
        button(cv, "PRINT", "", 0.f, 924.f, 97.84f, false);
        button(cv, "RECALL", "", 0.f, 974.f, 97.84f, false);
        const char* labels[5] = {"SEND", "STANBY", "UNABLE", "LOAD", "PRINT"};
        for (int i = 0; i < 5; i++)
            button(cv, labels[i], i == 3 ? "SEC3" : "", 631.76f, 768.f + 51.5f * i, 136.24f, true);
        icon_button(cv, 550.f, 768.f, true);
        icon_button(cv, 550.f, 808.f, false);
    }

    static void page_title(SkCanvas& cv, const std::string& t, float x, float y) {
        SkFont f = a380::font("Ecam", 36.f);
        float w = a380::text_width(f, t);
        a380::draw_text(cv, t, x, y, f, fill(kWhite));
        cv.drawRect(SkRect::MakeXYWH(x, y + 2.f, w, 3.6f), fill(kWhite));
    }

    void video(SkCanvas& cv) {
        page_title(cv, "VIDEO", 6, 29);
        text(cv, "NOT AVAIL", 384, 349.67f, 26, kAmber, Align::Center);
    }

    static void stroke_pts(SkCanvas& cv, std::initializer_list<SkPoint> pts, uint32_t c, float w, bool round,
                           bool closed = false) {
        SkPathBuilder b;
        bool first = true;
        for (auto& q : pts) {
            if (first) b.moveTo(q.fX, q.fY);
            else b.lineTo(q.fX, q.fY);
            first = false;
        }
        if (closed) b.close();
        SkPaint p = fill(c);
        p.setStyle(SkPaint::kStroke_Style);
        p.setStrokeWidth(w);
        if (round) {
            p.setStrokeCap(SkPaint::kRound_Cap);
            p.setStrokeJoin(SkPaint::kRound_Join);
        }
        a380::draw_path_cached(cv, b.detach(), p);
    }

    static void mid_text(SkCanvas& cv, const std::string& t, float x, float y, float px, uint32_t c, float ls) {
        SkFont f = a380::font("Ecam", px);
        if (c == kGreen || c == kAmber) {
            text(cv, t, x, y + a380::baseline_offset(f, a380::Baseline::Middle), px, c, Align::Center, ls);
            return;
        }
        a380::draw_text(cv, t, x, y, f, fill(c), Align::Center, a380::Baseline::Middle, ls);
    }

    void computer_shape(SkCanvas& cv, float x, float y, int num, bool failed) {
        cv.save();
        cv.translate(x, y);
        if (num == 1) stroke_pts(cv, {{0, 0}, {98, 0}, {98, -35}, {77, -35}}, failed ? kAmber : 0x4f5467, 3.f, true);
        else if (num == 2) stroke_pts(cv, {{0, 0}, {94, 0}, {94, -35}, {71, -35}}, failed ? kAmber : 0x4f5467, 3.f, true);
        else stroke_pts(cv, {{0, 0}, {93, 0}, {93, -35}, {70, -35}}, failed ? kAmber : 0x4f5467, 3.f, true);
        float tx = num == 1 ? 81.f : num == 2 ? 75.f : 74.f, ty = num == 2 ? -10.f : -8.f;
        text(cv, std::to_string(num), tx, ty, 26, failed ? kAmber : kGreen);
        cv.restore();
    }

    void computers(SkCanvas& cv, const char* label, float x, float y, float lx, float ly, int n, const bool* failed) {
        cv.save();
        cv.translate(x, y);
        mid_text(cv, label, lx, ly, 22, kWhite, 1.f);
        const float xs[3] = {8, 36, 62}, ys[3] = {100, 112, 124};
        for (int i = 0; i < n; i++) computer_shape(cv, xs[i], ys[i], i + 1, failed[i]);
        cv.restore();
    }

    void actuator(SkCanvas& cv, float x, float y, bool eha, bool avail, char letter) {
        cv.save();
        cv.translate(x, y);
        a380::chromium_fill_rect(cv, SkRect::MakeWH(25, 25), fill(0x4f5467));
        uint32_t c = avail ? kGreen : kAmber;
        if (eha) stroke_pts(cv, {{17, 6}, {10, 13}, {17, 13}, {10, 20}}, c, 4.f, true);
        else text(cv, std::string(1, letter), 6, 23, 26, c);
        cv.restore();
    }

    void slat_flap_act(SkCanvas& cv, const a380::Inputs& in, float x, float y, bool slats) {
        bool green = in.var("L:A32NX_HYD_GREEN_SYSTEM_1_SECTION_PRESSURE_SWITCH") != 0;
        bool yellow = in.var("L:A32NX_HYD_YELLOW_SYSTEM_1_SECTION_PRESSURE_SWITCH") != 0;
        bool ess = in.var("L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED") != 0;
        cv.save();
        cv.translate(x, y);
        stroke_pts(cv, {{10, 0}, {0, 0}, {0, 52}, {98, 52}, {98, 0}, {88, 0}}, kWhite, 1.f, true);
        a380::draw_text(cv, slats ? "SLATS" : "FLAPS", 13, 10, a380::font("Ecam", 23.f), fill(kWhite));
        actuator(cv, 19, 17, slats, slats ? ess : green, 'G');
        actuator(cv, 53, 17, false, slats ? green : yellow, slats ? 'G' : 'Y');
        cv.restore();
    }

    void vdefl(SkCanvas& cv, float x, float y, bool power, bool onGround, double deflection, bool droop) {
        cv.save();
        cv.translate(x, y);
        a380::chromium_fill_rect(cv, SkRect::MakeWH(15, 116), fill(0x4f5467));
        auto off = [](double d) { return float((d > 0 ? d / 20.0 : d / 30.0) * 58.0); };
        if (droop) {
            SkPaint p = fill(kWhite);
            p.setStyle(SkPaint::kStroke_Style);
            p.setStrokeWidth(2.f);
            cv.drawCircle(7.5f, off(5) + 58.f, 3.75f, p);
        }
        uint32_t c = power ? kGreen : kAmber;
        if (onGround && power) {
            stroke_pts(cv, {{0, -1}, {15, -1}}, kGreen, 2.f, false);
            stroke_pts(cv, {{0, 117}, {15, 117}}, kGreen, 2.f, false);
        }
        float d = off(deflection);
        a380::chromium_fill_rect(cv, SkRect::MakeLTRB(0, std::min(58.f, 58.f + d), 15, std::max(58.f, 58.f + d)), fill(c));
        stroke_pts(cv, {{0, 58}, {15, 58}}, c, 2.f, false);
        cv.restore();
    }

    void hdefl(SkCanvas& cv, float x, float y, bool power, bool onGround, double deflection) {
        cv.save();
        cv.translate(x, y);
        a380::chromium_fill_rect(cv, SkRect::MakeWH(116, 15), fill(0x4f5467));
        uint32_t c = power ? kGreen : kAmber;
        float lim = 58.f;
        if (onGround && power) {
            stroke_pts(cv, {{-1, 0}, {-1, 15}}, kGreen, 2.f, false);
            stroke_pts(cv, {{117, 0}, {117, 15}}, kGreen, 2.f, false);
            stroke_pts(cv, {{58 - lim + 4, 0}, {58 - lim - 1 + 4, 0}, {58 - lim - 1 + 4, 12}}, kGreen, 3.f, true);
            stroke_pts(cv, {{58 + lim - 4, 0}, {58 + lim + 5 - 4, 0}, {58 + lim + 5 - 4, 12}}, kGreen, 3.f, true);
        }
        float d = float(deflection / 30.0 * 58.0);
        a380::chromium_fill_rect(cv, SkRect::MakeLTRB(std::min(58.f, 58.f + d), 0, std::max(58.f, 58.f + d), 15), fill(c));
        stroke_pts(cv, {{58, 0}, {58, 15}}, c, 2.f, false);
        cv.restore();
    }

    void ebha(SkCanvas& cv, float x, float y, char letter, bool hyd, bool elec) {
        cv.save();
        cv.translate(x, y);
        a380::chromium_fill_rect(cv, SkRect::MakeWH(40, 25), fill(0x4f5467));
        text(cv, std::string(1, letter), 3, 23, 26, hyd ? kGreen : kAmber);
        stroke_pts(cv, {{34, 6}, {27, 13}, {34, 13}, {27, 20}}, elec ? kGreen : kAmber, 4.f, true);
        cv.restore();
    }

    double sampled(const a380::Inputs& in, const std::string& name, const char* unit = nullptr) {
        double now = in.now_ms();
        auto it = hold_.find(name);
        if (it == hold_.end() || now - it->second.second >= 100.0 || now < it->second.second) {
            double v = unit ? in.var(name, unit, 0.0) : in.var(name);
            hold_[name] = {v, now};
            return v;
        }
        return it->second.first;
    }

    void fctl_surfaces(SkCanvas& cv, const a380::Inputs& in) {
        bool green = in.var("L:A32NX_HYD_GREEN_SYSTEM_1_SECTION_PRESSURE_SWITCH") != 0;
        bool yellow = in.var("L:A32NX_HYD_YELLOW_SYSTEM_1_SECTION_PRESSURE_SWITCH") != 0;
        bool ess = in.var("L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED") != 0;
        bool ac1 = in.var("L:A32NX_ELEC_AC_1_BUS_IS_POWERED") != 0;
        bool eha = in.var("L:A32NX_ELEC_247XP_BUS_IS_POWERED") != 0;
        bool ground = in.var("L:A32NX_LGCIU_1_LEFT_GEAR_COMPRESSED") != 0;

        struct El { float x; bool left, outer; };
        for (El e : {El{196, true, true}, El{218, true, false}, El{536, false, false}, El{558, false, true}}) {
            bool hydAv = e.left ? green : yellow;
            bool ehaSide = (e.left && e.outer) || (!e.left && !e.outer);
            bool elecAv = ehaSide ? ac1 : ess;
            double d = sampled(in, std::string("L:A32NX_HYD_ELEVATOR_") + (e.left ? "LEFT" : "RIGHT") +
                                   (e.outer ? "_OUTWARD" : "_INWARD") + "_DEFLECTION");
            vdefl(cv, e.x, 212, hydAv || elecAv, ground, d * -30.0, false);
            float ax = ehaSide ? -13.f : -2.f;
            actuator(cv, e.x + ax, 212 + 131, false, hydAv, e.left ? 'G' : 'Y');
            actuator(cv, e.x + ax, 212 + 161, true, elecAv, 'E');
        }

        struct Ai { float x; bool left; int pos; };
        for (Ai a : {Ai{41, true, 2}, Ai{63, true, 1}, Ai{86, true, 0}, Ai{670, false, 0}, Ai{692, false, 1},
                     Ai{715, false, 2}}) {
            bool a1 = a.pos == 1 ? yellow : green;
            bool a2 = a.pos == 2 ? yellow : a.pos == 1 ? ess : ac1;
            const char* nm = a.pos == 2 ? "_OUTWARD" : a.pos == 1 ? "_MIDDLE" : "_INWARD";
            double d = sampled(in, std::string("L:A32NX_HYD_AILERON_") + (a.left ? "LEFT" : "RIGHT") + nm + "_DEFLECTION");
            vdefl(cv, a.x, 474, a1 || a2, ground, (a.left ? -d : d) * -30.0, true);
            float ax = a.pos == 1 ? -5.f : ((a.left && a.pos == 2) || (!a.left && a.pos == 0)) ? -13.f : 2.f;
            actuator(cv, a.x + ax, 474 + 128, false, a1, a.pos == 1 ? 'Y' : 'G');
            actuator(cv, a.x + ax, 474 + 159, a.pos != 2, a2, 'Y');
        }

        bool spoilPower = ground ? green : (green || ess);
        for (int side = 0; side < 2; side++)
            for (int i = 0; i < 8; i++) {
                int pos = side == 0 ? 8 - i : i + 1;
                float x = (side == 0 ? 147.f : 425.f) + 26.f * i;
                float yo = pos <= 2 ? 0.f : pos <= 4 ? -4.f : pos <= 6 ? -8.f : -12.f;
                float maxd = pos >= 3 ? 50.f : 35.f;
                double d = sampled(in, std::string("L:A32NX_HYD_SPOILER_") + std::to_string(pos) +
                                       (side == 0 ? "_LEFT" : "_RIGHT") + "_DEFLECTION");
                cv.save();
                cv.translate(x, 545 + yo);
                a380::chromium_fill_rect(cv, SkRect::MakeLTRB(0, -35, 15, 0), fill(0x4f5467));
                if (ground && spoilPower && pos >= 3)
                    stroke_pts(cv, {{0, 45 / maxd * -35 - 2}, {15, 45 / maxd * -35 - 2}}, kGreen, 2.f, false);
                float h = float(d * 50 / maxd * -35);
                a380::chromium_fill_rect(cv, SkRect::MakeLTRB(0, std::min(0.f, h), 15, std::max(0.f, h)),
                            fill(spoilPower ? kGreen : kAmber));
                if (!spoilPower) {
                    stroke_pts(cv, {{1, -2}, {1, -33}}, kAmber, 4.f, true);
                    stroke_pts(cv, {{14, -2}, {14, -33}}, kAmber, 4.f, true);
                }
                cv.restore();
            }

        for (int up = 1; up >= 0; up--) {
            bool upper = up == 1;
            bool ps1 = upper ? (ess || yellow) : (ess || green);
            bool ps2 = upper ? (eha || green) : (ac1 || yellow);
            double d = sampled(in, upper ? "L:A32NX_HYD_UPPER_RUDDER_DEFLECTION" : "L:A32NX_HYD_LOWER_RUDDER_DEFLECTION");
            float y = upper ? 108.f : 140.f;
            hdefl(cv, 327, y, ps1 || ps2, ground, d * 30.0);
            ebha(cv, 327 - 60, y + (upper ? -39.f : -2.f), upper ? 'Y' : 'G', upper ? yellow : green, ess);
            ebha(cv, 327 - 60, y + (upper ? -8.f : 30.f), upper ? 'G' : 'Y', upper ? green : yellow, upper ? eha : ac1);
        }

        cv.save();
        cv.translate(385, 123);
        text(cv, "RUD TRIM", 70, -8, 22, kWhite, Align::Left, 1.f);
        Arinc w1 = arinc(in.var("L:A32NX_SEC_1_RUDDER_STATUS_WORD"));
        Arinc w3 = arinc(in.var("L:A32NX_SEC_3_RUDDER_STATUS_WORD"));
        bool sec1Trim = w1.bit(28);
        bool valid = sec1Trim || w3.bit(28);
        if (valid) {
            Arinc t = arinc(sampled(in, sec1Trim ? "L:A32NX_SEC_1_RUDDER_ACTUAL_POSITION"
                                                  : "L:A32NX_SEC_3_RUDDER_ACTUAL_POSITION"));
            double tv = t.valid() ? t.f32() : 0.0;
            double tvMarker = std::isfinite(t.f32()) ? t.f32() : 0.0;
            bool pw = in.var("L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED") != 0 || in.var("L:A32NX_ELEC_DC_1_BUS_IS_POWERED") != 0;
            uint32_t c = pw ? kCyan : kAmber;
            cv.save();
            cv.translate(float(-tvMarker / 30.0 * 58.0), 0);
            poly(cv, {{0, 0}, {6, 8}, {0, 16}, {-6, 8}}, c);
            cv.restore();
            char b[16];
            std::snprintf(b, sizeof b, "%.1f", std::fabs(tv));
            if (std::fabs(tv) > 0.05) text(cv, tv > 0 ? "L" : "R", 72, 17, 22, c);
            text(cv, b, 159, 17, 22, c, Align::Right);
            text(cv, "\xC2\xB0", 159, 19, 22, kCyan);
        } else {
            text(cv, "XX", 94, 17, 22, kAmber);
            text(cv, "X", -7, 18, 22, kAmber);
        }
        cv.restore();

        cv.save();
        cv.translate(385, 273);
        bool hyd = green || yellow;
        double th = sampled(in, "ELEVATOR TRIM POSITION", "radians") * 57.29577951308232;
        stroke_pts(cv, {{0, 0}, {0, 119}}, kWhite, 4.f, true);
        stroke_pts(cv, {{-10, 0}, {10, 0}}, kWhite, 4.f, true);
        stroke_pts(cv, {{-10, 118}, {10, 118}}, kWhite, 4.f, true);
        stroke_pts(cv, {{-10, 98}, {10, 98}}, kWhite, 4.f, true);
        cv.save();
        cv.translate(0, float(-th * 10));
        stroke_pts(cv, {{-5, 98}, {-26, 87}, {-26, 110}}, hyd ? kGreen : kAmber, 2.f, true, true);
        cv.restore();
        uint32_t tc = hyd ? kWhite : kAmber;
        mid_text(cv, "PITCH", 57, -6, 22, tc, 1.f);
        mid_text(cv, "TRIM", 57, 17, 22, tc, 1.f);
        uint32_t vc = hyd ? kGreen : kAmber;
        int tenths = int(std::fabs(th) * 10 + 0.5);
        text(cv, std::to_string(tenths / 10), 38, 67, 26, vc, Align::Right);
        text(cv, ".", 37, 67, 26, vc);
        text(cv, std::to_string(tenths % 10), 55, 67, 26, vc);
        text(cv, "\xC2\xB0", 65, 68, 26, kCyan);
        if (std::fabs(th) > 0.05) text(cv, th > 0 ? "UP" : "DN", 82, 68, 26, vc);
        actuator(cv, -63, 24, false, green, 'G');
        actuator(cv, -63, 66, false, yellow, 'Y');
        cv.restore();
    }

    static std::vector<std::string> sts_list(const a380::Inputs& in, const char* topic) {
        std::vector<std::string> r;
        const auto* j = in.bus(topic);
        if (j && j->is_array())
            for (auto& e : *j) {
                if (e.is_string()) r.push_back(e.get<std::string>());
                else if (e.is_number()) r.push_back(std::to_string(e.get<long long>()));
            }
        return r;
    }

    template <size_t N>
    static const StsMsg* sts_find(const StsMsg (&tab)[N], const std::string& id) {
        for (auto& m : tab)
            if (id == m.id) return &m;
        return nullptr;
    }

    static float snx(float x) {
        return std::floor(x * 1.5f + 0.5f) / 1.5f;
    }

    static float sts_ls() {
        return 1.75f;
    }

    static uint32_t sts_color(int c) {
        switch (c) {
            case 1: return 0xffff00;
            case 2: return 0xff0000;
            case 3: return kGreen;
            case 4: return kAmber;
            case 5: return kCyan;
            case 6: return 0xff94ff;
            default: return kWhite;
        }
    }

    template <size_t N>
    static void sts_lines(SkCanvas& cv, const StsMsg (&tab)[N], const std::vector<std::string>& ids, float x, float top) {
        cv.save();
        cv.clipRect(SkRect::MakeLTRB(0, 0, 753, 1000));
        for (size_t i = 0; i < ids.size(); i++) {
            const StsMsg* m = sts_find(tab, ids[i]);
            if (m) text(cv, m->text, snx(x), top + 24.17f + 30.f * float(i), 26, sts_color(m->color), Align::Left, sts_ls());
        }
        cv.restore();
    }

    static void sts_heading(SkCanvas& cv, const std::string& t, float y0, bool lines) {
        SkFont f = a380::font("Ecam", 28.f);
        float w = a380::text_width(f, t, 1.75f);
        float base = y0 + 24.37f;
        float cx = 384.f;
        a380::draw_text(cv, t, cx - w * 0.5f, base, f, fill(kWhite), Align::Left, a380::Baseline::Alphabetic, 1.75f);
        float ub = std::floor(base * 1.5f) + 3.f;
        snapped_rect(cv, SkRect::MakeXYWH(cx - w * 0.5f, ub / 1.5f, w, 4.f / 1.5f), kWhite);
        if (lines) {
            float l = cx - w * 0.5f - 12.f, r = cx + w * 0.5f + 12.f;
            snapped_rect(cv, SkRect::MakeLTRB(15.f, y0 + 11.f, l, y0 + 13.f), 0xc4c6cf);
            snapped_rect(cv, SkRect::MakeLTRB(r, y0 + 11.f, 753.f, y0 + 13.f), 0xc4c6cf);
        }
    }

    static void sts_divided(SkCanvas& cv, float y0, const std::vector<StsMsg const*>& l, const std::vector<StsMsg const*>& r,
                            size_t rows) {
        SkFont f = a380::font("Ecam", 26.f);
        const float lc = 195.25f, rc = 572.75f;
        a380::draw_text(cv, "ALL PHASES", lc - a380::text_width(f, "ALL PHASES", 1.75f) * 0.5f, y0 + 59.67f, f, fill(kWhite),
                        Align::Left, a380::Baseline::Alphabetic, 1.75f);
        a380::draw_text(cv, "APPR & LDG", rc - a380::text_width(f, "APPR & LDG", 1.75f) * 0.5f, y0 + 59.67f, f, fill(kWhite),
                        Align::Left, a380::Baseline::Alphabetic, 1.75f);
        float top = y0 + 72.3f;
        snapped_rect(cv, SkRect::MakeXYWH(375.5f, y0 + 37.7f, 2.f, 34.6f + float(rows) * 30.f + 3.f), 0xc4c6cf);
        cv.save();
        cv.clipRect(SkRect::MakeLTRB(0, 0, 375.33f, 1000));
        for (size_t i = 0; i < l.size(); i++)
            if (l[i]) text(cv, l[i]->text, snx(15.f), top + 24.67f + 30.f * float(i), 26, sts_color(l[i]->color), Align::Left, sts_ls());
        cv.restore();
        cv.save();
        cv.clipRect(SkRect::MakeLTRB(0, 0, 753, 1000));
        for (size_t i = 0; i < r.size(); i++)
            if (r[i]) text(cv, r[i]->text, snx(392.5f), top + 24.67f + 30.f * float(i), 26, sts_color(r[i]->color), Align::Left, sts_ls());
        cv.restore();
    }

    static void more_label(SkCanvas& cv) {
        SkFont f = a380::font("Ecam", 26.f);
        float w = a380::text_width(f, "MORE");
        a380::draw_text(cv, "MORE", 172, 28, f, fill(kWhite));
        cv.drawRect(SkRect::MakeXYWH(172, 28 + 2.f * 26.f / 36.f, w, 3.6f * 26.f / 36.f), fill(kWhite));
    }

    void sts(SkCanvas& cv, const a380::Inputs& in) {
        page_title(cv, "STATUS", 6, 29);
        const bool moreActive = in.var("L:A32NX_ECAM_SD_MORE_SHOWN") != 0;
        if (moreActive) more_label(cv);
        auto flag = [&](const char* n) { return in.var(n, {}, 1.0) != 0; };
        const bool fwsAvail = (flag("L:A32NX_FWS1_IS_HEALTHY") && (flag("L:A32NX_AFDX_4_3_REACHABLE") || flag("L:A32NX_AFDX_14_13_REACHABLE"))) ||
                              (flag("L:A32NX_FWS2_IS_HEALTHY") && (flag("L:A32NX_AFDX_4_4_REACHABLE") || flag("L:A32NX_AFDX_14_14_REACHABLE")));
        if (!fwsAvail) {
            text(cv, "STATUS NOT AVAILABLE", 384, 340, 30, kAmber, Align::Center);
            return;
        }
        auto limAll = sts_list(in, "fws_limitations_all_phases"), limApp = sts_list(in, "fws_limitations_appr_ldg");
        auto info = sts_list(in, "fws_information");
        auto inopAll = sts_list(in, "fws_inop_sys_all_phases"), inopApp = sts_list(in, "fws_inop_sys_appr_ldg");
        auto redund = sts_list(in, "fws_inop_sys_redundancy_loss");
        const auto* dj = in.bus("fws_deferred_procedures");
        const size_t deferredCount = dj && dj->is_array() ? dj->size() : 0;
        if (limAll.empty() && limApp.empty() && deferredCount == 0 && info.empty() && inopAll.empty() && inopApp.empty()) {
            text(cv, "NORMAL", 384, 340, 30, kGreen, Align::Center);
            return;
        }
        auto find = [](auto& tab, const std::vector<std::string>& ids) {
            std::vector<const StsMsg*> v;
            for (auto& id : ids) v.push_back(sts_find(tab, id));
            return v;
        };
        const int usable = 13;
        const int pageToShow = int(in.var("L:A32NX_ECAM_SD_STS_PAGE_TO_SHOW"));
        if (moreActive) {
            size_t from = size_t(std::max(0, pageToShow)) * usable;
            std::vector<std::string> slice;
            for (size_t i = from; i < redund.size() && i < from + usable; i++) slice.push_back(redund[i]);
            if (!slice.empty()) {
                sts_heading(cv, "INOP SYS REDUND", 45.f, false);
                sts_lines(cv, kInop, slice, 15.f, 45.f + 37.7f);
            }
            if (redund.size() > from + usable) {
                snapped_rect(cv, SkRect::MakeXYWH(0, 615.f, 768, 40.f), kWhite);
                a380::draw_text(cv, "PRESS MORE FOR NEXT MORE PAGE", 384, 645.33f, a380::font("Ecam", 26.f), fill(0x000000),
                                Align::Center);
            }
            return;
        }
        std::vector<const StsMsg*> deferred;
        if (dj && dj->is_array())
            for (auto& e : *dj)
                if (e.is_object() && e.contains("id") && e["id"].is_string())
                    if (auto* m = sts_find(kDeferred, e["id"].get<std::string>())) deferred.push_back(m);
        size_t limRows = std::max(limAll.size(), limApp.size());
        size_t inopRows = std::max(inopAll.size(), inopApp.size());
        int defStrings = 0;
        for (int t = 0; t < 4; t++) {
            int n = 0;
            for (auto* m : deferred) n += m->color == t;
            if (n > 0) defStrings += n + 1;
        }
        int used = int(limRows) + 2;
        int defLines = defStrings + 1, infoLines = int(info.size()) + 1, inopLines = int(inopRows) + 2;
        int defPage, infoPage, inopPage, altPage, page = 0;
        bool defSep = true, infoSep = true, inopSep = true;
        if (used + defLines > usable) { defPage = ++page; used = 0; defSep = false; }
        else defPage = page;
        used += defLines;
        if (used + infoLines > usable) { infoPage = ++page; used = 0; infoSep = false; }
        else infoPage = page;
        used += infoLines;
        if (used + inopLines > usable) { inopPage = ++page; used = 0; inopSep = false; }
        else inopPage = page;
        used += inopLines;
        if (used > usable) altPage = ++page;
        else altPage = page;
        bool nextShown = false;
        auto section = [&](int secPage) {
            if (pageToShow == secPage) return 1;
            if (pageToShow + 1 == secPage && !nextShown) {
                nextShown = true;
                return 2;
            }
            return 0;
        };
        const int defSt = section(defPage), infoSt = section(infoPage), inopSt = section(inopPage);
        const bool next = nextShown || section(altPage) == 2;
        float y = 45.f;
        if (limRows > 0 && pageToShow == 0) {
            sts_heading(cv, "LIMITATIONS", y, false);
            sts_divided(cv, y, find(kLimits, limAll), find(kLimits, limApp), limRows);
            y += 72.3f + float(limRows) * 30.f + 3.f + 4.f;
        }
        if (defSt == 2) {
            sts_heading(cv, "DEFERRED PROCEDURE LIST", y, true);
            text(cv, "DISPLAYED ON NEXT STATUS PAGE...", snx(15.f), y + 37.7f + 24.0f, 26, kAmber, Align::Left, sts_ls());
            y += 37.7f + 63.f;
        }
        if (defSt == 1 && !deferred.empty()) {
            static const char* kTypes[4] = {"ALL PHASES", "AT TOP OF DESCENT", "FOR APPROACH", "FOR LANDING"};
            sts_heading(cv, "DEFERRED PROCEDURE LIST", y, defSep);
            SkFont f26 = a380::font("Ecam", 26.f);
            float chevron = a380::text_width(f26, "> ", sts_ls());
            float ly = y + 37.7f + 24.0f;
            for (int t = 0; t < 4; t++) {
                bool any = false;
                for (auto* m : deferred) {
                    if (m->color != t) continue;
                    if (!any) {
                        text(cv, std::string(kTypes[t]) + " :", snx(15.f), ly, 26, kAmber, Align::Left, sts_ls());
                        ly += 30.f;
                        any = true;
                    }
                    text(cv, "> ", snx(15.f), ly, 26, kWhite, Align::Left, sts_ls());
                    text(cv, m->text, snx(15.f) + chevron, ly, 26, kAmber, Align::Left, sts_ls());
                    ly += 30.f;
                }
            }
            y += 37.7f + float(deferred.size()) * 60.f + 3.f;
        }
        if (infoSt == 2) {
            sts_heading(cv, "INFO", y, true);
            text(cv, "DISPLAYED ON NEXT STATUS PAGE...", snx(15.f), y + 37.7f + 24.0f, 26, kGreen, Align::Left, sts_ls());
            y += 37.7f + 63.f;
        }
        if (infoSt == 1 && !info.empty()) {
            sts_heading(cv, "INFO", y, infoSep);
            sts_lines(cv, kInfos, info, 15.f, y + 37.7f);
            y += 37.7f + float(info.size()) * 30.f + 3.f;
        }
        if (inopSt == 2) {
            sts_heading(cv, "INOP SYS", y, true);
            text(cv, "DISPLAYED ON NEXT STATUS PAGE...", snx(15.f), y + 37.7f + 24.0f, 26, kAmber, Align::Left, sts_ls());
        }
        if (inopSt == 1 && inopRows > 0) {
            sts_heading(cv, "INOP SYS", y, inopSep);
            sts_divided(cv, y, find(kInop, inopAll), find(kInop, inopApp), inopRows);
        }
        if (next) {
            snapped_rect(cv, SkRect::MakeXYWH(0, 575.f, 768, 40.f), kWhite);
            a380::draw_text(cv, "PRESS STS FOR NEXT STATUS PAGE", 384, 605.33f, a380::font("Ecam", 26.f), fill(0x000000),
                            Align::Center);
        }
        if (!redund.empty()) {
            outset_box(cv, SkRect::MakeXYWH(335.333f, 626.f, 68.f, 27.333f), 2.f);
            a380::draw_text(cv, "MORE", 369.f, 649.333f, a380::font("Ecam", 24.f), fill(kWhite), Align::Center);
        }
    }

    void fctl(SkCanvas& cv, const a380::Inputs& in) {
        page_title(cv, "F/CTL", 6, 29);
        draw_image(cv, "SD_FCTL_WING.png", -2, 518, 774, 204);
        draw_image(cv, "SD_FCTL_TAIL.png", 100, -15, 570, 570);
        bool pf[3], sf[3], nf[3] = {false, false, false};
        for (int i = 0; i < 3; i++) {
            pf[i] = in.var("L:A32NX_PRIM_" + std::to_string(i + 1) + "_HEALTHY") == 0;
            sf[i] = in.var("L:A32NX_SEC_" + std::to_string(i + 1) + "_HEALTHY") == 0;
        }
        computers(cv, "PRIM", 0, 0, 45, 85, 3, pf);
        computers(cv, "SEC", 0, 78, 48, 84, 3, sf);
        computers(cv, "SLATS", 609, 9, 46, 85, 2, nf);
        computers(cv, "FLAPS", 609, 78, 46, 84, 2, nf);
        fctl_surfaces(cv, in);
        slat_flap_act(cv, in, 336, 437, true);
        slat_flap_act(cv, in, 336, 594, false);
    }

    static std::string fixed0(double v) {
        char b[32];
        std::snprintf(b, sizeof b, "%.0f", v);
        return b;
    }

    static int jsr(double v) { return int(std::floor(v + 0.5)); }

    void underlined(SkCanvas& cv, const std::string& s, float x, float y, float px) {
        text(cv, s, x, y, px, kWhite);
        SkFont f = a380::font("Ecam", px);
        float w = a380::text_width(f, s);
        cv.drawRect(SkRect::MakeXYWH(x, y + 2.f * px / 36.f, w, 3.6f * px / 36.f), fill(kWhite));
    }

    void crz(SkCanvas& cv, const a380::Inputs& in) {
        page_title(cv, "CRUISE", 6, 29);
        auto P = [&](const char* d, float w) { sdsys::stroke_path(cv, sdsys::svg_path(d), kWhite, w, true); };
        cv.save();
        cv.translate(0, -4);
        P("M325.04,565.99l0.98,42.94h64.27c3.35,0,6.05-2.74,6-6.09l-1.81-123.07c-0.05-3.28-2.72-5.91-6-5.91H123.91 v133.75", 4.f);
        stroke_pts(cv, {{65.11f, 608.93f}, {191.61f, 608.93f}, {191.61f, 568.14f}}, kWhite, 4.f, true);
        stroke_pts(cv, {{123.91f, 522.06f}, {394.39f, 522.06f}}, kWhite, 4.f, true);
        stroke_pts(cv, {{123.91f, 566.65f}, {394.39f, 566.65f}}, kWhite, 4.f, true);
        P("M123.91,473.85c-21.31-1-38.87,7.37-51.53,18.82", 4.f);
        P("M65.11,608.93c-15.76-2.46-30.88-10.29-45.08-25.76c-7.68-8.03-10.07-15.85-2.34-26.13 c1.62-2.15,3.41-4.16,5.45-5.93c6.8-5.92,16.95-14.76,27.28-23.76", 4.f);
        sdsys::fill_path(cv, sdsys::svg_path("M76.81,488.95v9.88h-9.22C70.17,493.52,73.22,490.12,76.81,488.95z"), kWhite);
        poly(cv, {{53.58f, 520.52f}, {53.58f, 531.26f}, {46.61f, 531.26f}}, kWhite);
        P("M71.89,492.21 c-8.54,10.04 -16.3,22.2 -23.45,35.98", 2.f);
        cv.restore();

        underlined(cv, "FUEL", 15, 80, 29);
        for (float lx : {130.f, 285.f, 451.f, 601.f}) stroke_pts(cv, {{lx, 117.f}, {lx + 27.f, 117.f}}, kWhite, 3.f, false);
        const float ffX[4] = {112, 263, 578, 730}, fuX[4] = {122, 273, 588, 740};
        int fuTotal = 0;
        for (int i = 0; i < 4; i++) {
            std::string e = std::to_string(i + 1);
            int ff = jsr(in.var("L:A32NX_ENGINE_FF:" + e) / 10.0) * 10;
            int fu = jsr(in.var("L:A32NX_FUEL_USED:" + e) / 50.0) * 50;
            fuTotal += fu;
            text(cv, fixed0(ff), ffX[i], 127, 29, kGreen, Align::Right, 1.f);
            text(cv, fixed0(fu), fuX[i], 227, 29, kGreen, Align::Right, 1.f);
        }
        text(cv, "FF", 383, 99, 26, kWhite, Align::Center);
        text(cv, "KG/H", 383, 131, 22, kCyan, Align::Center);
        for (float lx : {134.f, 285.f, 451.f, 601.f}) stroke_pts(cv, {{lx, 217.f}, {lx + 27.f, 217.f}}, kWhite, 3.f, false);
        text(cv, "FU", 383, 199, 26, kWhite, Align::Center);
        text(cv, "TOTAL", 383, 228, 26, kWhite, Align::Center);
        text(cv, fixed0(fuTotal), 428, 265, 29, kGreen, Align::Right, 1.f);
        text(cv, "KG", 383, 285, 22, kCyan, Align::Center);
        underlined(cv, "AIR", 18, 330, 29);

        auto flag = [&](const char* n) { return in.var(n, {}, 1.0) != 0; };
        const bool altAuto = flag("L:A32NX_OVHD_PRESS_MAN_ALTITUDE_PB_IS_AUTO");
        const bool vsAuto = flag("L:A32NX_OVHD_PRESS_MAN_VS_CTL_PB_IS_AUTO");
        const bool manual = !altAuto || !vsAuto;
        Arinc fm1 = arinc(in.var("L:A32NX_FM1_LANDING_ELEVATION")), fm2 = arinc(in.var("L:A32NX_FM2_LANDING_ELEVATION"));
        if (!manual && (fm1.normal() || fm2.normal())) {
            double le = fm1.normal() ? fm1.f32() : fm2.f32();
            text(cv, "LDG ELEVN", 470, 355, 26, kWhite, Align::Center, 1.f);
            text(cv, fixed0(jsr(le / 50.0) * 50), 653, 359, 29, kGreen, Align::Right);
            text(cv, "FT", 658, 359, 22, kCyan);
        }
        int cpcs = 0;
        for (int i = 1; i <= 4 && cpcs == 0; i++)
            if (arinc(in.var("L:A32NX_COND_CPIOM_B" + std::to_string(i) + "_CPCS_DISCRETE_WORD")).normal()) cpcs = i;
        std::string cs = std::to_string(cpcs);
        Arinc altA = arinc(in.var("L:A32NX_PRESS_CABIN_ALTITUDE_B" + cs)), dpA = arinc(in.var("L:A32NX_PRESS_CABIN_DELTA_PRESSURE_B" + cs)),
              vsA = arinc(in.var("L:A32NX_PRESS_CABIN_VS_B" + cs));
        const double cabAlt = altA.normal() ? altA.f32() : in.var("L:A32NX_PRESS_MAN_CABIN_ALTITUDE");
        const double deltaP = dpA.normal() ? dpA.f32() : in.var("L:A32NX_PRESS_MAN_CABIN_DELTA_PRESSURE");
        const double cabVs = vsA.normal() ? vsA.f32() : in.var("L:A32NX_PRESS_MAN_CABIN_VS");

        if (manual) {
            const float gx = 555.f, gy = 465.f, gr = 50.f;
            sdsys::arc_path(cv, gx, gy, gr, 160, 20, kWhite, 2.f);
            struct VM {
                float val;
                const char* label;
                float nx, ny;
            };
            for (VM m : {VM{2.f, "2", -5.f, 25.f}, VM{1.f, nullptr, 0.f, 0.f}, VM{0.f, "0", 10.f, 10.f}, VM{-1.f, nullptr, 0.f, 0.f},
                         VM{-2.f, "2", -5.f, -10.f}}) {
                sdsys::gauge_marker(cv, gx, gy, m.val, -2, 2, gr, 180, 0, kWhite, 1.f, false);
                if (m.label) {
                    float dx, dy;
                    sdsys::gauge_dir(m.val, -2, 2, 180, 0, dx, dy);
                    text(cv, m.label, gx + dx * gr * 0.85f + m.nx, gy + dy * gr * 0.85f + m.ny, 20, kWhite);
                }
            }
            float vsGauge = std::fabs(cabVs / 1000.0) <= 2.25 ? float(cabVs / 1000.0) : 2.25f;
            sdsys::gauge_marker(cv, gx, gy, vsGauge, -2, 2, gr, 180, 0, kGreen, 3.f, true, true);
        }

        text(cv, "DELTA P", 175, 425, 26, kWhite, Align::Left, 1.f);
        char dp[32];
        std::snprintf(dp, sizeof dp, "%.1f", deltaP);
        std::string dps = dp;
        size_t dot = dps.find('.');
        text(cv, dps.substr(0, dot), 332, 425, 29, kGreen, Align::Right);
        text(cv, ".", 348, 425, 29, kGreen, Align::Right);
        text(cv, dot == std::string::npos ? "" : dps.substr(dot + 1), 350, 425, 29, kGreen);
        text(cv, "PSI", 374, 425, 22, kCyan);

        if (vsAuto) text(cv, "AUTO", 522, 450, 24, kGreen, Align::Left, 1.f);
        text(cv, "CAB V/S", 606, 450, 24, kWhite, Align::Left, 2.f);
        int vsRounded = jsr(cabVs / 50.0) * 50;
        text(cv, fixed0(manual ? vsRounded : std::abs(vsRounded)), 660, 484, 29, kGreen, Align::Right);
        text(cv, "FT/MIN", 664, 484, 22, kCyan);

        if (altAuto) text(cv, "AUTO", 520, 585, 24, kGreen, Align::Left, 1.f);
        text(cv, "CAB ALT", 605, 585, 24, kWhite, Align::Left, 2.f);
        double ldgElev = fm1.normal() ? fm1.f32() : fm2.normal() ? fm2.f32() : 0.0;
        int altRounded = jsr(cabAlt / 50.0) * 50;
        text(cv, fixed0(altRounded > 0 ? altRounded : 0), 652, 616, 29, (cabAlt > 9550 && ldgElev < 9550) ? kRed : kGreen, Align::Right);
        text(cv, "FT", 661, 616, 22, kCyan);

        if ((cabVs * 60 <= -25 || cabVs * 60 >= 25) && !manual) {
            cv.save();
            if (cabVs * 60 <= -25) {
                cv.translate(130, 870);
                cv.scale(1, -1);
            } else {
                cv.translate(130, 77);
            }
            sdsys::stroke_path(cv, sdsys::svg_path("M433,405 h7 L446,395"), kGreen, 2.f, false);
            cv.save();
            cv.rotate(38.f, 452.f, 388.f);
            stroke_pts(cv, {{452.f, 388.f}, {447.f, 396.f}, {457.f, 396.f}}, kGreen, 2.f, false, true);
            cv.restore();
            cv.restore();
        }

        text(cv, "\xC2\xB0" "C", 19, 506, 22, kCyan);
        auto temp = [&](const std::string& n) { return in.var("L:A32NX_COND_" + n + "_TEMP"); };
        double ckpt = in.var("L:A32NX_COND_CKPT_TEMP");
        text(cv, fixed0(ckpt), 109, 528, 29, kGreen, Align::Right);
        double minMain = 1e30, maxMain = -1e30, minUp = 1e30, maxUp = -1e30;
        for (int i = 1; i <= 8; i++) {
            double t = temp("MAIN_DECK_" + std::to_string(i));
            minMain = std::min(minMain, t);
            maxMain = std::max(maxMain, t);
        }
        for (int i = 1; i <= 7; i++) {
            double t = temp("UPPER_DECK_" + std::to_string(i));
            minUp = std::min(minUp, t);
            maxUp = std::max(maxUp, t);
        }
        text(cv, fixed0(minUp), 210, 506, 29, kGreen, Align::Right);
        text(cv, "TO", 239, 506, 24, kWhite, Align::Left, 2.f);
        text(cv, fixed0(maxUp), 340, 506, 29, kGreen, Align::Right);
        text(cv, fixed0(minMain), 210, 552, 29, kGreen, Align::Right);
        text(cv, "TO", 239, 552, 24, kWhite, Align::Left, 2.f);
        text(cv, fixed0(maxMain), 340, 552, 29, kGreen, Align::Right);
        text(cv, fixed0(ckpt), 178, 595, 29, kGreen, Align::Right);
        text(cv, fixed0(ckpt), 378, 595, 29, kGreen, Align::Right);
    }

    void draw(SkCanvas& cv, const a380::Inputs& in) override {
        if (!du_.update(cv, in, size())) return;
        int page = int(in.var("L:A32NX_ECAM_SD_PAGE_TO_SHOW", {}, 0.0));
        if (page == 13) crz(cv, in);
        if (page == 15) video(cv);
        if (page == 11) fctl(cv, in);
        if (page == 14) sts(cv, in);
        permanent(cv, in);
        mailbox(cv);
    }
    a380::CdsDisplayUnit du_{"93", {"DC_2"}};
    std::map<std::string, std::pair<double, double>> hold_;
};

std::unique_ptr<a380::Display> make(std::string_view) { return std::make_unique<Sd>(); }

}  // namespace

A380_REGISTER_DISPLAY("sdv2", make);
