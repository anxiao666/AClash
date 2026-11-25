/** Match Clash-style node names to ISO country codes. Order matters. */
const REGION_RULES: Array<{ code: string; re: RegExp }> = [
  { code: "hk", re: /香港|Hong\s*Kong|\bHKG\b|\bHK\b/i },
  { code: "mo", re: /澳门|澳門|Macao|Macau|\bMO\b|\bMAC\b/i },
  { code: "tw", re: /台湾|台灣|Taiwan|\bTWN\b|\bTW\b/i },
  { code: "sg", re: /新加坡|Singapore|狮城|\bSGP\b|\bSG\b/i },
  { code: "jp", re: /日本|Japan|东京|大阪|Tokyo|Osaka|\bJPN\b|\bJP\b/i },
  { code: "kr", re: /韩国|韓國|Korea|首尔|首爾|\bKOR\b|\bKR\b/i },
  { code: "us", re: /美国|美國|United\s*States|\bUSA\b|\bUS\b|洛杉矶|纽约|硅谷|西雅图/i },
  { code: "ca", re: /加拿大|Canada|Toronto|Vancouver|\bCAN\b|\bCA\b/i },
  { code: "gb", re: /英国|英國|United\s*Kingdom|London|\bUK\b|\bGBR?\b/i },
  { code: "de", re: /德国|德國|Germany|法兰克福|\bDEU\b|\bDE\b/i },
  { code: "fr", re: /法国|法國|France|Paris|\bFRA\b|\bFR\b/i },
  { code: "nl", re: /荷兰|荷蘭|Netherlands|Amsterdam|\bNLD\b|\bNL\b/i },
  { code: "au", re: /澳大利亚|澳洲|Australia|Sydney|\bAUS\b|\bAU\b/i },
  { code: "ru", re: /俄罗斯|俄羅斯|Russia|Moscow|\bRUS\b|\bRU\b/i },
  { code: "ua", re: /乌克兰|烏克蘭|Ukraine|Kyiv|Kiev|\bUKR\b|\bUA\b/i },
  { code: "md", re: /摩尔多瓦|摩爾多瓦|Moldova|\bMDA\b|\bMD\b/i },
  { code: "cl", re: /智利|Chile|\bCHL\b|\bCL\b/i },
  { code: "pk", re: /巴基斯坦|Pakistan|\bPAK\b|\bPK\b/i },
  { code: "il", re: /以色列|Israel|Tel\s*Aviv|\bISR\b|\bIL\b/i },
  { code: "ro", re: /罗马尼亚|羅馬尼亞|Romania|\bROU\b|\bRO\b/i },
  { code: "cz", re: /捷克|Czech|\bCZE\b|\bCZ\b/i },
  { code: "at", re: /奥地利|奧地利|Austria|Vienna|\bAUT\b|\bAT\b/i },
  { code: "be", re: /比利时|比利時|Belgium|\bBEL\b|\bBE\b/i },
  { code: "pt", re: /葡萄牙|Portugal|\bPRT\b|\bPT\b/i },
  { code: "fi", re: /芬兰|芬蘭|Finland|Helsinki|\bFIN\b|\bFI\b/i },
  { code: "no", re: /挪威|Norway|\bNOR\b|\bNO\b/i },
  { code: "dk", re: /丹麦|丹麥|Denmark|\bDNK\b|\bDK\b/i },
  { code: "gr", re: /希腊|希臘|Greece|\bGRC\b|\bGR\b/i },
  { code: "hu", re: /匈牙利|Hungary|\bHUN\b|\bHU\b/i },
  { code: "bg", re: /保加利亚|保加利亞|Bulgaria|\bBGR\b|\bBG\b/i },
  { code: "hr", re: /克罗地亚|克羅地亞|Croatia|\bHRV\b|\bHR\b/i },
  { code: "rs", re: /塞尔维亚|塞爾維亞|Serbia|\bSRB\b|\bRS\b/i },
  { code: "sk", re: /斯洛伐克|Slovakia|\bSVK\b|\bSK\b/i },
  { code: "si", re: /斯洛文尼亚|斯洛文尼亞|Slovenia|\bSVN\b|\bSI\b/i },
  { code: "lt", re: /立陶宛|Lithuania|\bLTU\b|\bLT\b/i },
  { code: "lv", re: /拉脱维亚|拉脫維亞|Latvia|\bLVA\b|\bLV\b/i },
  { code: "ee", re: /爱沙尼亚|愛沙尼亞|Estonia|\bEST\b|\bEE\b/i },
  { code: "by", re: /白俄罗斯|白俄羅斯|Belarus|\bBLR\b|\bBY\b/i },
  { code: "kz", re: /哈萨克斯坦|哈薩克|Kazakhstan|\bKAZ\b|\bKZ\b/i },
  { code: "uz", re: /乌兹别克|烏茲別克|Uzbekistan|\bUZB\b|\bUZ\b/i },
  { code: "ge", re: /格鲁吉亚|格魯吉亞|Georgia|\bGEO\b|\bGE\b/i },
  { code: "az", re: /阿塞拜疆|Azerbaijan|\bAZE\b|\bAZ\b/i },
  { code: "am", re: /亚美尼亚|亞美尼亞|Armenia|\bARM\b|\bAM\b/i },
  { code: "iq", re: /伊拉克|Iraq|\bIRQ\b|\bIQ\b/i },
  { code: "ir", re: /伊朗|Iran|\bIRN\b|\bIR\b/i },
  { code: "sa", re: /沙特|Saudi|\bSAU\b|\bSA\b/i },
  { code: "eg", re: /埃及|Egypt|\bEGY\b|\bEG\b/i },
  { code: "ma", re: /摩洛哥|Morocco|\bMAR\b|\bMA\b/i },
  { code: "ke", re: /肯尼亚|肯亞|Kenya|\bKEN\b|\bKE\b/i },
  { code: "pe", re: /秘鲁|秘魯|Peru|\bPER\b|\bPE\b/i },
  { code: "co", re: /哥伦比亚|哥倫比亞|Colombia|\bCOL\b|\bCO\b/i },
  { code: "mx", re: /墨西哥|Mexico|\bMEX\b|\bMX\b/i },
  { code: "uy", re: /乌拉圭|烏拉圭|Uruguay|\bURY\b|\bUY\b/i },
  { code: "ec", re: /厄瓜多尔|厄瓜多爾|Ecuador|\bECU\b|\bEC\b/i },
  { code: "cr", re: /哥斯达黎加|哥斯達黎加|Costa\s*Rica|\bCRI\b|\bCR\b/i },
  { code: "pa", re: /巴拿马|巴拿馬|Panama|\bPAN\b|\bPA\b/i },
  { code: "cu", re: /古巴|Cuba|\bCUB\b|\bCU\b/i },
  { code: "jm", re: /牙买加|牙買加|Jamaica|\bJAM\b|\bJM\b/i },
  { code: "np", re: /尼泊尔|尼泊爾|Nepal|\bNPL\b|\bNP\b/i },
  { code: "lk", re: /斯里兰卡|斯里蘭卡|Sri\s*Lanka|\bLKA\b|\bLK\b/i },
  { code: "bd", re: /孟加拉|Bangladesh|\bBGD\b|\bBD\b/i },
  { code: "mm", re: /缅甸|緬甸|Myanmar|Burma|\bMMR\b|\bMM\b/i },
  { code: "kh", re: /柬埔寨|Cambodia|\bKHM\b|\bKH\b/i },
  { code: "la", re: /老挝|寮國|Laos|\bLAO\b|\bLA\b/i },
  { code: "mn", re: /蒙古|Mongolia|\bMNG\b|\bMN\b/i },
  { code: "in", re: /印度|India|Mumbai|\bIND\b|\bIN\b/i },
  { code: "th", re: /泰国|泰國|Thailand|曼谷|\bTHA\b|\bTH\b/i },
  { code: "vn", re: /越南|Vietnam|\bVNM\b|\bVN\b/i },
  { code: "my", re: /马来|馬來|Malaysia|\bMYS\b|\bMY\b/i },
  { code: "ph", re: /菲律宾|菲律賓|Philippines|\bPHL\b|\bPH\b/i },
  { code: "id", re: /印尼|印度尼西亚|Indonesia|\bIDN\b|\bID\b/i },
  { code: "tr", re: /土耳其|Turkey|\bTUR\b|\bTR\b/i },
  { code: "br", re: /巴西|Brazil|\bBRA\b|\bBR\b/i },
  { code: "ar", re: /阿根廷|Argentina|\bARG\b|\bAR\b/i },
  { code: "ae", re: /阿联酋|阿聯酋|Dubai|UAE|\bARE\b/i },
  { code: "ng", re: /尼日利亚|Nigeria|\bNGA\b|\bNG\b/i },
  { code: "za", re: /南非|South\s*Africa|\bZAF\b|\bZA\b/i },
  { code: "nz", re: /新西兰|紐西蘭|New\s*Zealand|\bNZL\b|\bNZ\b/i },
  { code: "it", re: /意大利|Italy|Milan|\bITA\b|\bIT\b/i },
  { code: "es", re: /西班牙|Spain|\bESP\b|\bES\b/i },
  { code: "ch", re: /瑞士|Switzerland|Zurich|\bCHE\b|\bCH\b/i },
  { code: "se", re: /瑞典|Sweden|\bSWE\b|\bSE\b/i },
  { code: "pl", re: /波兰|波蘭|Poland|\bPOL\b|\bPL\b/i },
  { code: "ie", re: /爱尔兰|愛爾蘭|Ireland|Dublin|\bIRL\b|\bIE\b/i },
  { code: "cn", re: /中国|大陸|回国|China|\bCHN\b|\bCN\b|北京|上海|深圳|广州|杭州/i },
];

const FLAG_RE = /[\u{1F1E6}-\u{1F1FF}]{2}/u;

function codeFromFlagEmoji(flag: string): string {
  const chars = [...flag];
  if (chars.length < 2) return "";
  const a = chars[0].codePointAt(0);
  const b = chars[1].codePointAt(0);
  if (a == null || b == null) return "";
  return String.fromCharCode(a - 0x1f1e6 + 65, b - 0x1f1e6 + 65).toLowerCase();
}

/** Returns lowercase ISO country code, or empty string. */
export function regionCodeForNodeName(name: string): string {
  if (!name) return "";
  const existing = name.match(FLAG_RE);
  if (existing) {
    const code = codeFromFlagEmoji(existing[0]);
    if (code) return code;
  }
  for (const rule of REGION_RULES) {
    if (rule.re.test(name)) return rule.code;
  }
  return "";
}

export function flagSrcForCode(code: string): string {
  if (!code) return "";
  return `/flags/${code.toLowerCase()}.png`;
}
