====================================================================================================
### macro_china_urban_unemployment
# FILE: /home/redtramp/Green/Language/miniconda3/lib/python3.13/site-packages/akshare/economic/macro_china.py
def macro_china_urban_unemployment() -> pd.DataFrame:
    """
    国家统计局-月度数据-城镇调查失业率
    https://data.stats.gov.cn/dg/website/page.html#/pc/national/monthData
    :return: 城镇调查失业率
    :rtype: pandas.DataFrame
    """
    url = (
        "https://data.stats.gov.cn/dg/website/publicrelease/web/external/stream/esData"
    )
    headers = {
        "Accept": "application/json, text/plain, */*",
        "Content-Type": "application/json;charset=UTF-8",
        "Origin": "https://data.stats.gov.cn",
        "Referer": "https://data.stats.gov.cn/dg/website/page.html#/pc/national/monthData",
        "User-Agent": (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
            "AppleWebKit/537.36 (KHTML, like Gecko) "
            "Chrome/138.0.0.0 Safari/537.36"
        ),
    }
    payload = {
        "cid": "ee3b7046b390415b9b7745e3d16f6052",
        "indicatorIds": [
            "3888eac6062945a79c8a27e5f13d4953",
            "1d550f3ec77a463bb607d4a3427e1465",
            "1c1b2d9ab24048bfadc5c7d9510dc663",
            "3921da310de24f14b6457c235657baf9",
            "bd6da1abb26046c2acb38aa701d90e86",
            "7bc1bd5daeac48ae8bb413c34ece1d08",
            "c03a36c9562246b6bc8aab010951ef1c",
            "1061f276ce354907b0b9900c266cf851",
            "40ab91b1ef4948e89633c5c7f55b9713",
        ],
        "daCatalogId": "",
        "das": [{"text": "全国", "value": "000000000000"}],
        "dts": ["199001MM-203601MM"],
        "showType": "1",
        "rootId": "fc982599aa684be7969d7b90b1bd0e84",
    }
    r = curl_requests.post(
        url, json=payload, headers=headers, impersonate="chrome", timeout=30
    )
    data_json = r.json()
    if not data_json.get("success") or "data" not in data_json:
        return pd.DataFrame(columns=["date", "item", "value"])
    data_list = []
    for month_item in data_json["data"]:
        raw_month = month_item["name"]
        year_part = raw_month.split("年")[0]
        month_part = raw_month.split("年")[1].replace("月", "")
        month_clean = year_part + month_part.zfill(2)
        for value_item in month_item["values"]:
            if value_item["_name"] == "城镇调查失业率":
                rate = value_item["value"]
                if rate:
                    indicator_clean = value_item["i_showname"].replace(" (%)", "")
                    data_list.append([month_clean, indicator_clean, rate])
    temp_df = pd.DataFrame(data_list, columns=["date", "item", "value"])
    temp_df.sort_values(by=["date"], ascending=True, inplace=True)
    temp_df.reset_index(drop=True, inplace=True)
    return temp_df

====================================================================================================
### macro_cnbs
# FILE: /home/redtramp/Green/Language/miniconda3/lib/python3.13/site-packages/akshare/economic/marco_cnbs.py
def macro_cnbs() -> pd.DataFrame:
    """
    国家金融与发展实验室-中国宏观杠杆率数据
    http://114.115.232.154:8080/
    :return: 中国宏观杠杆率数据
    :rtype: pandas.DataFrame
    """
    url = "http://114.115.232.154:8080/handler/download.ashx"
    temp_df = pd.read_excel(
        url, sheet_name="Data", header=0, skiprows=1, engine="openpyxl"
    )

    temp_df["Period"] = pd.to_datetime(temp_df["Period"]).dt.strftime("%Y-%m")
    temp_df.dropna(axis=1, inplace=True)

    temp_df.rename(
        columns={
            "Period": "年份",
            "Household": "居民部门",
            "Non-financial corporations": "非金融企业部门",
            "Central government ": "中央政府",
            "Local government": "地方政府",
            "General government": "政府部门",
            "Non financial sector": "实体经济部门",
            "Financial sector(asset side)": "金融部门资产方",
            "Financial sector(liability side)": "金融部门负债方",
        },
        inplace=True,
    )

    column_order = [
        "年份",
        "居民部门",
        "非金融企业部门",
        "政府部门",
        "中央政府",
        "地方政府",
        "实体经济部门",
        "金融部门资产方",
        "金融部门负债方",
    ]
    temp_df = temp_df.reindex(columns=column_order)
    temp_df["居民部门"] = pd.to_numeric(temp_df["居民部门"], errors="coerce")
    temp_df["非金融企业部门"] = pd.to_numeric(
        temp_df["非金融企业部门"], errors="coerce"
    )
    temp_df["政府部门"] = pd.to_numeric(temp_df["政府部门"], errors="coerce")
    temp_df["中央政府"] = pd.to_numeric(temp_df["中央政府"], errors="coerce")
    temp_df["地方政府"] = pd.to_numeric(temp_df["地方政府"], errors="coerce")
    temp_df["实体经济部门"] = pd.to_numeric(temp_df["实体经济部门"], errors="coerce")
    temp_df["金融部门资产方"] = pd.to_numeric(
        temp_df["金融部门资产方"], errors="coerce"
    )
    temp_df["金融部门负债方"] = pd.to_numeric(
        temp_df["金融部门负债方"], errors="coerce"
    )
    return temp_df

====================================================================================================
### macro_fx_sentiment
# FILE: /home/redtramp/Green/Language/miniconda3/lib/python3.13/site-packages/akshare/economic/macro_other.py
def macro_fx_sentiment(
    start_date: str = "20221011", end_date: str = "20221017"
) -> pd.DataFrame:
    """
    金十数据-外汇-投机情绪报告
    外汇投机情绪报告显示当前市场多空仓位比例，数据由8家交易平台提供，涵盖11个主要货币对和1个黄金品种。
    报告内容: 品种: 澳元兑日元、澳元兑美元、欧元兑美元、欧元兑澳元、欧元兑日元、英镑兑美元、英镑兑日元、纽元兑美元、美元兑加元、美元兑瑞郎、美元兑日元以及现货黄金兑美元。
             数据: 由Shark - fx整合全球8家交易平台（ 包括 Oanda、 FXCM、 Insta、 Dukas、 MyFxBook以及FiboGroup） 的多空投机仓位数据而成。
    名词释义: 外汇投机情绪报告显示当前市场多空仓位比例，数据由8家交易平台提供，涵盖11个主要货币对和1个黄金品种。
    工具使用策略: Shark-fx声明表示，基于“主流通常都是错误的”的事实，当空头头寸超过60%，交易者就应该建立多头仓位； 同理，当市场多头头寸超过60%，交易者则应该建立空头仓位。此外，当多空仓位比例接近50%的情况下，我们则倾向于建议交易者不要进场，保持观望。
    https://datacenter.jin10.com/reportType/dc_ssi_trends
    :param start_date: 具体交易日
    :type start_date: str
    :param end_date: 具体交易日, 与 end_date 相同
    :type end_date: str
    :return: 投机情绪报告
    :rtype: pandas.DataFrame
    """
    start_date = "-".join([start_date[:4], start_date[4:6], start_date[6:]])
    end_date = "-".join([end_date[:4], end_date[4:6], end_date[6:]])
    url = "https://datacenter-api.jin10.com/sentiment/datas"
    params = {
        "start_date": start_date,
        "end_date": end_date,
        "currency_pair": "",
    }
    headers = {
        "accept": "*/*",
        "accept-encoding": "",
        "accept-language": "zh-CN,zh;q=0.9,en;q=0.8",
        "cache-control": "no-cache",
        "origin": "https://datacenter.jin10.com",
        "pragma": "no-cache",
        "referer": "https://datacenter.jin10.com/reportType/dc_ssi_trends",
        "sec-fetch-mode": "cors",
        "sec-fetch-site": "same-site",
        "user-agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) "
        "Chrome/79.0.3945.130 Safari/537.36",
        "x-app-id": "rU6QIu7JHe2gOUeR",
        "x-csrf-token": "",
        "x-version": "1.0.0",
    }
    r = requests.get(url, params=params, headers=headers)
    data_json = r.json()
    temp_df = pd.DataFrame(data_json["data"]["values"]).T
    temp_df.reset_index(inplace=True)
    temp_df.rename(columns={"index": "date"}, inplace=True)
    for col in temp_df.columns[1:]:
        temp_df[col] = pd.to_numeric(temp_df[col], errors="coerce")
    return temp_df

====================================================================================================
### macro_global_sox_index
# FILE: /home/redtramp/Green/Language/miniconda3/lib/python3.13/site-packages/akshare/economic/macro_china.py
def macro_global_sox_index() -> pd.DataFrame:
    """
    费城半导体指数
    https://data.eastmoney.com/cjsj/hyzs_list_EMI00055562.html
    :return: 费城半导体指数
    :rtype: pandas.DataFrame
    """
    url = "https://datacenter-web.eastmoney.com/api/data/v1/get"
    params = {
        "sortColumns": "REPORT_DATE",
        "sortTypes": "-1",
        "pageSize": "500",
        "pageNumber": "1",
        "reportName": "RPT_INDUSTRY_INDEX",
        "columns": "REPORT_DATE,INDICATOR_VALUE,CHANGE_RATE,CHANGERATE_3M,CHANGERATE_6M,CHANGERATE_1Y,"
        "CHANGERATE_2Y,CHANGERATE_3Y",
        "filter": '(INDICATOR_ID="EMI00055562")',
        "source": "WEB",
        "client": "WEB",
    }
    r = requests.get(url, params=params)
    data_json = r.json()
    total_page = data_json["result"]["pages"]
    big_df = pd.DataFrame()
    tqdm = get_tqdm()
    for page in tqdm(range(1, total_page + 1), leave=False):
        params.update({"pageNumber": page})
        r = requests.get(url, params=params)
        data_json = r.json()
        temp_df = pd.DataFrame(data_json["result"]["data"])
        big_df = pd.concat([big_df, temp_df], ignore_index=True)
    big_df.drop_duplicates(inplace=True)
    big_df.columns = [
        "日期",
        "最新值",
        "涨跌幅",
        "近3月涨跌幅",
        "近6月涨跌幅",
        "近1年涨跌幅",
        "近2年涨跌幅",
        "近3年涨跌幅",
    ]
    big_df["日期"] = pd.to_datetime(big_df["日期"]).dt.date
    big_df["最新值"] = pd.to_numeric(big_df["最新值"])
    big_df["涨跌幅"] = pd.to_numeric(big_df["涨跌幅"])
    big_df["近3月涨跌幅"] = pd.to_numeric(big_df["近3月涨跌幅"])
    big_df["近6月涨跌幅"] = pd.to_numeric(big_df["近6月涨跌幅"])
    big_df["近1年涨跌幅"] = pd.to_numeric(big_df["近1年涨跌幅"])
    big_df["近2年涨跌幅"] = pd.to_numeric(big_df["近2年涨跌幅"])
    big_df["近3年涨跌幅"] = pd.to_numeric(big_df["近3年涨跌幅"])
    big_df.sort_values(["日期"], inplace=True)
    big_df.reset_index(inplace=True, drop=True)
    return big_df

====================================================================================================
### macro_info_ws
# FILE: /home/redtramp/Green/Language/miniconda3/lib/python3.13/site-packages/akshare/economic/macro_info_ws.py
def macro_info_ws(date: str = "20240514") -> pd.DataFrame:
    """
    华尔街见闻-日历-宏观
    https://wallstreetcn.com/calendar
    :param date: 日期
    :type date: str
    :return: 日历-宏观
    :rtype: pandas.DataFrame
    """
    date = __convert_date_format(date)
    url = "https://api-one-wscn.awtmt.com/apiv1/finance/macrodatas"
    datetime_obj = datetime.strptime(date, "%Y-%m-%d %H:%M:%S")
    one_day = timedelta(days=1)
    new_datetime = datetime_obj + one_day
    date_str = new_datetime.strftime("%Y-%m-%d %H:%M:%S")
    params = {"start": __format_date(date), "end": __format_date(date_str)}
    r = requests.get(url, params=params)
    data_json = r.json()
    temp_df = pd.DataFrame(data_json["data"]["items"])
    temp_df["public_date"] = pd.to_datetime(
        temp_df["public_date"], errors="coerce", unit="s", utc=True
    ).dt.tz_convert("Asia/Shanghai")
    temp_df["public_date"] = temp_df["public_date"].dt.strftime("%Y-%m-%d %H:%M:%S")
    temp_df = temp_df.rename(
        columns={
            "public_date": "时间",
            "country": "地区",
            "title": "事件",
            "importance": "重要性",
            "actual": "今值",
            "forecast": "预期",
            "previous": "前值",
            "revised": "修正",
            "uri": "链接",
        }
    )
    temp_df = temp_df[
        [
            "时间",
            "地区",
            "事件",
            "重要性",
            "今值",
            "预期",
            "前值",
            "修正",
            "链接",
        ]
    ]
    temp_df["今值"] = pd.to_numeric(temp_df["今值"], errors="coerce")
    temp_df["预期"] = pd.to_numeric(temp_df["预期"], errors="coerce")
    temp_df["前值"] = pd.to_numeric(temp_df["前值"], errors="coerce")
    temp_df["修正"] = pd.to_numeric(temp_df["修正"], errors="coerce")
    temp_df["前值"] = np.where(
        temp_df["修正"].notnull(), temp_df["修正"], temp_df["前值"]
    )
    del temp_df["修正"]
    return temp_df

====================================================================================================
### macro_rmb_deposit
# FILE: /home/redtramp/Green/Language/miniconda3/lib/python3.13/site-packages/akshare/economic/macro_finance_ths.py
def macro_rmb_deposit() -> pd.DataFrame:
    """
    同花顺-数据中心-宏观数据-人民币存款余额
    https://data.10jqka.com.cn/macro/rmb/
    :return: 人民币存款余额
    :rtype: pandas.DataFrame
    """
    url = "https://data.10jqka.com.cn/macro/rmb/"
    headers = {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
    }
    r = requests.get(url, headers=headers)
    temp_df = pd.read_html(StringIO(r.text), skiprows=0)[0]
    temp_df.columns = [
        "月份",
        "新增存款-数量",
        "新增存款-同比",
        "新增存款-环比",
        "新增企业存款-数量",
        "新增企业存款-同比",
        "新增企业存款-环比",
        "新增储蓄存款-数量",
        "新增储蓄存款-同比",
        "新增储蓄存款-环比",
        "新增其他存款-数量",
        "新增其他存款-同比",
        "新增其他存款-环比",
    ]
    temp_df["新增存款-数量"] = pd.to_numeric(temp_df["新增存款-数量"], errors="coerce")
    temp_df["新增企业存款-数量"] = pd.to_numeric(
        temp_df["新增企业存款-数量"], errors="coerce"
    )
    temp_df["新增企业存款-数量"] = pd.to_numeric(
        temp_df["新增企业存款-数量"], errors="coerce"
    )
    temp_df["新增储蓄存款-数量"] = pd.to_numeric(
        temp_df["新增储蓄存款-数量"], errors="coerce"
    )
    temp_df["新增其他存款-数量"] = pd.to_numeric(
        temp_df["新增其他存款-数量"], errors="coerce"
    )
    temp_df.sort_values(by=["月份"], inplace=True, ignore_index=True)
    return temp_df

====================================================================================================
### macro_rmb_loan
# FILE: /home/redtramp/Green/Language/miniconda3/lib/python3.13/site-packages/akshare/economic/macro_finance_ths.py
def macro_rmb_loan() -> pd.DataFrame:
    """
    同花顺-数据中心-宏观数据-新增人民币贷款
    https://data.10jqka.com.cn/macro/loan/
    :return: 新增人民币贷款
    :rtype: pandas.DataFrame
    """
    url = "https://data.10jqka.com.cn/macro/loan/"
    headers = {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
    }
    r = requests.get(url, headers=headers)
    temp_df = pd.read_html(StringIO(r.text), skiprows=0)[0]
    temp_df.columns = [
        "月份",
        "新增人民币贷款-总额",
        "新增人民币贷款-同比",
        "新增人民币贷款-环比",
        "累计人民币贷款-总额",
        "累计人民币贷款-同比",
    ]
    temp_df["新增人民币贷款-总额"] = pd.to_numeric(
        temp_df["新增人民币贷款-总额"], errors="coerce"
    )
    temp_df["累计人民币贷款-总额"] = pd.to_numeric(
        temp_df["累计人民币贷款-总额"], errors="coerce"
    )
    temp_df.sort_values(by=["月份"], inplace=True, ignore_index=True)
    return temp_df

====================================================================================================
### macro_stock_finance
# FILE: /home/redtramp/Green/Language/miniconda3/lib/python3.13/site-packages/akshare/economic/macro_finance_ths.py
def macro_stock_finance() -> pd.DataFrame:
    """
    同花顺-数据中心-宏观数据-股票筹资
    https://data.10jqka.com.cn/macro/finance/
    :return: 股票筹资
    :rtype: pandas.DataFrame
    """
    url = "https://data.10jqka.com.cn/macro/finance/"
    headers = {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
    }
    r = requests.get(url, headers=headers)
    temp_df = pd.read_html(StringIO(r.text))[0]
    temp_df.rename(
        columns={
            "月份": "月份",
            "募集资金(亿元)": "募集资金",
            "首发募集资金(亿元)": "首发募集资金",
            "增发募集资金(亿元)": "增发募集资金",
            "配股募集资金(亿元)": "配股募集资金",
        },
        inplace=True,
    )
    temp_df = temp_df[
        ["月份", "募集资金", "首发募集资金", "增发募集资金", "配股募集资金"]
    ]
    temp_df["募集资金"] = pd.to_numeric(temp_df["募集资金"], errors="coerce")
    temp_df["首发募集资金"] = pd.to_numeric(temp_df["首发募集资金"], errors="coerce")
    temp_df["增发募集资金"] = pd.to_numeric(temp_df["增发募集资金"], errors="coerce")
    temp_df["配股募集资金"] = pd.to_numeric(temp_df["配股募集资金"], errors="coerce")
    temp_df.sort_values(by=["月份"], inplace=True, ignore_index=True)
    return temp_df

====================================================================================================
### macro_usa_cftc_c_holding
# FILE: /home/redtramp/Green/Language/miniconda3/lib/python3.13/site-packages/akshare/economic/macro_usa.py
def macro_usa_cftc_c_holding() -> pd.DataFrame:
    """
    美国商品期货交易委员会CFTC商品类非商业持仓报告, 数据区间从 19830107-至今
    https://datacenter.jin10.com/reportType/dc_cftc_c_report
    :return: 美国商品期货交易委员会CFTC外汇类非商业持仓报告
    :rtype: pandas.DataFrame
    """
    t = time.time()
    params = {"_": str(int(round(t * 1000)))}
    r = requests.get(
        url="https://cdn.jin10.com/data_center/reports/cftc_2.json", params=params
    )
    json_data = r.json()
    temp_df = pd.DataFrame(json_data["values"]).T
    temp_df.fillna(value="[0, 0, 0]", inplace=True)
    big_df = pd.DataFrame()
    for item in temp_df.columns:
        for i in range(3):
            inner_temp_df = temp_df.loc[:, item].apply(lambda x: eval(str(x))[i])
            inner_temp_df.name = inner_temp_df.name + "-" + json_data["keys"][i]["name"]
            big_df = pd.concat(objs=[big_df, inner_temp_df], axis=1)
    big_df = big_df.astype("float")
    big_df.reset_index(inplace=True)
    big_df.rename(columns={"index": "日期"}, inplace=True)
    big_df.sort_values(by=["日期"], ignore_index=True, inplace=True)
    return big_df

====================================================================================================
### macro_usa_cftc_merchant_currency_holding
# FILE: /home/redtramp/Green/Language/miniconda3/lib/python3.13/site-packages/akshare/economic/macro_usa.py
def macro_usa_cftc_merchant_currency_holding() -> pd.DataFrame:
    """
    美国商品期货交易委员会CFTC外汇类商业持仓报告, 数据区间从 19860115-至今
    https://datacenter.jin10.com/reportType/dc_cftc_merchant_currency
    :return: 美国商品期货交易委员会CFTC外汇类商业持仓报告
    :rtype: pandas.DataFrame
    """
    t = time.time()
    params = {"_": str(int(round(t * 1000)))}
    r = requests.get(
        url="https://cdn.jin10.com/data_center/reports/cftc_3.json", params=params
    )
    json_data = r.json()
    temp_df = pd.DataFrame(json_data["values"]).T
    temp_df.fillna(value="[0, 0, 0]", inplace=True)
    big_df = pd.DataFrame()
    for item in temp_df.columns:
        for i in range(3):
            inner_temp_df = temp_df.loc[:, item].apply(lambda x: eval(str(x))[i])
            inner_temp_df.name = inner_temp_df.name + "-" + json_data["keys"][i]["name"]
            big_df = pd.concat(objs=[big_df, inner_temp_df], axis=1)
    big_df = big_df.astype("float")
    big_df.reset_index(inplace=True)
    big_df.rename(columns={"index": "日期"}, inplace=True)
    big_df.sort_values(by=["日期"], ignore_index=True, inplace=True)
    return big_df

====================================================================================================
### macro_usa_cftc_merchant_goods_holding
# FILE: /home/redtramp/Green/Language/miniconda3/lib/python3.13/site-packages/akshare/economic/macro_usa.py
def macro_usa_cftc_merchant_goods_holding() -> pd.DataFrame:
    """
    美国商品期货交易委员会CFTC商品类商业持仓报告, 数据区间从 19860115-至今
    https://datacenter.jin10.com/reportType/dc_cftc_merchant_goods
    :return: 美国商品期货交易委员会CFTC商品类商业持仓报告
    :rtype: pandas.DataFrame
    """
    t = time.time()
    params = {"_": str(int(round(t * 1000)))}
    r = requests.get(
        url="https://cdn.jin10.com/data_center/reports/cftc_1.json", params=params
    )
    json_data = r.json()
    temp_df = pd.DataFrame(json_data["values"]).T
    temp_df.fillna(value="[0, 0, 0]", inplace=True)
    big_df = pd.DataFrame()
    for item in temp_df.columns:
        for i in range(3):
            inner_temp_df = temp_df.loc[:, item].apply(lambda x: eval(str(x))[i])
            inner_temp_df.name = inner_temp_df.name + "-" + json_data["keys"][i]["name"]
            big_df = pd.concat(objs=[big_df, inner_temp_df], axis=1)
    big_df = big_df.astype("float")
    big_df.reset_index(inplace=True)
    big_df.rename(columns={"index": "日期"}, inplace=True)
    big_df.sort_values(by=["日期"], ignore_index=True, inplace=True)
    return big_df

====================================================================================================
### macro_usa_cftc_nc_holding
# FILE: /home/redtramp/Green/Language/miniconda3/lib/python3.13/site-packages/akshare/economic/macro_usa.py
def macro_usa_cftc_nc_holding() -> pd.DataFrame:
    """
    美国商品期货交易委员会CFTC外汇类非商业持仓报告, 数据区间从 19830107-至今
    https://datacenter.jin10.com/reportType/dc_cftc_nc_report
    :return: 美国商品期货交易委员会CFTC外汇类非商业持仓报告
    :rtype: pandas.DataFrame
    """
    t = time.time()
    params = {"_": str(int(round(t * 1000)))}
    r = requests.get(
        url="https://cdn.jin10.com/data_center/reports/cftc_4.json", params=params
    )
    json_data = r.json()
    temp_df = pd.DataFrame(json_data["values"]).T
    temp_df.fillna(value="[0, 0, 0]", inplace=True)
    big_df = pd.DataFrame()
    for item in temp_df.columns:
        for i in range(3):
            inner_temp_df = temp_df.loc[:, item].apply(lambda x: eval(str(x))[i])
            inner_temp_df.name = inner_temp_df.name + "-" + json_data["keys"][i]["name"]
            big_df = pd.concat(objs=[big_df, inner_temp_df], axis=1)
    big_df = big_df.astype("float")
    big_df.reset_index(inplace=True)
    big_df.rename(columns={"index": "日期"}, inplace=True)
    big_df.sort_values(by=["日期"], ignore_index=True, inplace=True)
    return big_df

====================================================================================================
### macro_usa_cme_merchant_goods_holding
# FILE: /home/redtramp/Green/Language/miniconda3/lib/python3.13/site-packages/akshare/economic/macro_usa.py
def macro_usa_cme_merchant_goods_holding():
    """
    CME-贵金属, 数据区间从 20180405-至今
    https://datacenter.jin10.com/org
    :return: CME-贵金属
    :rtype: pandas.DataFrame
    """
    t = time.time()
    params = {"_": str(int(round(t * 1000)))}
    r = requests.get(
        url="https://cdn.jin10.com/data_center/reports/cme_3.json", params=params
    )
    json_data = r.json()
    big_df = pd.DataFrame()
    for item in json_data["values"].keys():
        temp_df = pd.DataFrame(json_data["values"][item])
        temp_df["日期"] = item
        big_df = pd.concat(objs=[big_df, temp_df], ignore_index=True)

    big_df.columns = ["pz", "tc", "-", "-", "-", "成交量", "-", "-", "日期"]
    big_df["品种"] = big_df["pz"] + "-" + big_df["tc"]
    big_df = big_df[["日期", "品种", "成交量"]]
    big_df.sort_values(["日期"], ignore_index=True, inplace=True)
    return big_df

====================================================================================================
### macro_usa_cpi_yoy
# FILE: /home/redtramp/Green/Language/miniconda3/lib/python3.13/site-packages/akshare/economic/macro_usa.py
def macro_usa_cpi_yoy() -> pd.DataFrame:
    """
    东方财富-经济数据一览-美国-CPI年率, 数据区间从 2008-至今
    https://data.eastmoney.com/cjsj/foreign_0_12.html
    :return: 美国 CPI 年率报告
    :rtype: pandas.DataFrame
    """
    url = "https://datacenter-web.eastmoney.com/api/data/v1/get"
    params = {
        "reportName": "RPT_ECONOMICVALUE_USA",
        "columns": "ALL",
        "filter": '(INDICATOR_ID="EMG00000733")',
        "sortColumns": "REPORT_DATE",
        "sortTypes": "-1",
        "source": "WEB",
        "client": "WEB",
    }
    r = requests.get(url, params=params)
    data_json = r.json()
    data_list = data_json["result"]["data"]
    temp_df = pd.DataFrame(
        data_list, columns=["REPORT_DATE", "PUBLISH_DATE", "VALUE", "PRE_VALUE"]
    )
    temp_df.columns = [
        "时间",
        "发布日期",
        "现值",
        "前值",
    ]
    temp_df["时间"] = pd.to_datetime(temp_df["时间"], errors="coerce").dt.date
    temp_df["发布日期"] = pd.to_datetime(temp_df["发布日期"], errors="coerce").dt.date
    temp_df["前值"] = pd.to_numeric(temp_df["前值"], errors="coerce")
    temp_df["现值"] = pd.to_numeric(temp_df["现值"], errors="coerce")
    temp_df.sort_values(by=["时间"], inplace=True, ignore_index=True)
    return temp_df

====================================================================================================
### macro_usa_crude_inner
# FILE: /home/redtramp/Green/Language/miniconda3/lib/python3.13/site-packages/akshare/economic/macro_usa.py
def macro_usa_crude_inner() -> pd.DataFrame:
    """
    美国原油产量报告, 数据区间从 19830107-至今
    https://datacenter.jin10.com/reportType/dc_eia_crude_oil_produce
    :return: 美国原油产量报告
    :rtype: pandas.DataFrame
    """
    t = time.time()
    params = {"_": t}
    res = requests.get(
        url="https://cdn.jin10.com/data_center/reports/usa_oil.json", params=params
    )
    temp_df = pd.DataFrame(res.json().get("values")).T
    big_df = pd.DataFrame()
    big_df["美国国内原油总量-产量"] = temp_df["美国国内原油总量"].apply(lambda x: x[0])
    big_df["美国国内原油总量-变化"] = temp_df["美国国内原油总量"].apply(lambda x: x[1])
    big_df["美国本土48州原油产量-产量"] = temp_df["美国本土48州原油产量"].apply(
        lambda x: x[0]
    )
    big_df["美国本土48州原油产量-变化"] = temp_df["美国本土48州原油产量"].apply(
        lambda x: x[1]
    )
    big_df["美国阿拉斯加州原油产量-产量"] = temp_df["美国阿拉斯加州原油产量"].apply(
        lambda x: x[0]
    )
    big_df["美国阿拉斯加州原油产量-变化"] = temp_df["美国阿拉斯加州原油产量"].apply(
        lambda x: x[1]
    )
    big_df = big_df.astype("float")
    big_df.reset_index(inplace=True)
    big_df.rename(columns={"index": "日期"}, inplace=True)
    big_df.sort_values(by=["日期"], ignore_index=True, inplace=True)
    return big_df

====================================================================================================
### macro_usa_phs
# FILE: /home/redtramp/Green/Language/miniconda3/lib/python3.13/site-packages/akshare/economic/macro_usa.py
def macro_usa_phs() -> pd.DataFrame:
    """
    东方财富-经济数据一览-美国-未决房屋销售月率
    https://data.eastmoney.com/cjsj/foreign_0_5.html
    :return: 未决房屋销售月率
    :rtype: pandas.DataFrame
    """
    url = "https://datacenter-web.eastmoney.com/api/data/v1/get"
    params = {
        "reportName": "RPT_ECONOMICVALUE_USA",
        "columns": "ALL",
        "filter": '(INDICATOR_ID="EMG00342249")',
        "pageNumber": "1",
        "pageSize": "2000",
        "sortColumns": "REPORT_DATE",
        "sortTypes": "-1",
        "source": "WEB",
        "client": "WEB",
        "p": "1",
        "pageNo": "1",
        "pageNum": "1",
    }
    r = requests.get(url, params=params)
    data_json = r.json()
    temp_df = pd.DataFrame(data_json["result"]["data"])
    temp_df.columns = [
        "-",
        "-",
        "-",
        "时间",
        "-",
        "发布日期",
        "现值",
        "前值",
    ]
    temp_df = temp_df[
        [
            "时间",
            "前值",
            "现值",
            "发布日期",
        ]
    ]
    temp_df["前值"] = pd.to_numeric(temp_df["前值"], errors="coerce")
    temp_df["现值"] = pd.to_numeric(temp_df["现值"], errors="coerce")
    temp_df["发布日期"] = pd.to_datetime(temp_df["发布日期"], errors="coerce").dt.date
    return temp_df

====================================================================================================
### macro_usa_rig_count
# FILE: /home/redtramp/Green/Language/miniconda3/lib/python3.13/site-packages/akshare/economic/macro_usa.py
def macro_usa_rig_count() -> pd.DataFrame:
    """
    贝克休斯钻井报告, 数据区间从 20080317-至今
    https://datacenter.jin10.com/reportType/dc_rig_count_summary
    :return: 贝克休斯钻井报告-当周
    :rtype: pandas.DataFrame
    """
    t = time.time()
    params = {"_": t}
    res = requests.get(
        url="https://cdn.jin10.com/data_center/reports/baker.json", params=params
    )
    temp_df = pd.DataFrame(res.json().get("values")).T
    big_df = pd.DataFrame()
    big_df["钻井总数_钻井数"] = temp_df["钻井总数"].apply(lambda x: x[0])
    big_df["钻井总数_变化"] = temp_df["钻井总数"].apply(lambda x: x[1])
    big_df["美国石油钻井_钻井数"] = temp_df["美国石油钻井"].apply(lambda x: x[0])
    big_df["美国石油钻井_变化"] = temp_df["美国石油钻井"].apply(lambda x: x[1])
    big_df["混合钻井_钻井数"] = temp_df["混合钻井"].apply(lambda x: x[0])
    big_df["混合钻井_变化"] = temp_df["混合钻井"].apply(lambda x: x[1])
    big_df["美国天然气钻井_钻井数"] = temp_df["美国天然气钻井"].apply(lambda x: x[0])
    big_df["美国天然气钻井_变化"] = temp_df["美国天然气钻井"].apply(lambda x: x[1])
    big_df = big_df.astype("float")
    big_df.reset_index(inplace=True)
    big_df.rename(columns={"index": "日期"}, inplace=True)
    big_df.sort_values(by=["日期"], inplace=True, ignore_index=True)
    return big_df

