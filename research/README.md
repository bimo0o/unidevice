# UniDevice: исследование производителей, протоколов и параметров

**Pet-проект, полностью написанный нейросетью под надзором пользователя.**

Срез: **23 октября 2026 года**. Исследование выполнено без подключённых устройств,
без установки фирменных программ, драйверов и без отправки HID-команд.

Результат — база для расширения UniDevice, а не заявление о готовой поддержке
перечисленных устройств. Закрытые внутренности фирменных приложений не выдаются за
изученный исходный код. Там, где доступны только публичные функции интерфейса,
способ передачи команд и их побочные эффекты остаются неизвестными.

## Что собрано

- [Поисковый каталог](catalog.html): открой файл в браузере; работает локально,
  без сети и библиотек. Ищи модель, VID/PID, `dpi`, `set_poll_rate`, `brightness`.
- [JSON](device-catalog.json) и [CSV для таблиц](device-catalog.csv): параметры,
  исходные поля, идентификаторы, оговорки и ссылки.
- [Модели по документации производителей](vendor-models.json): отдельный слой
  заявленных функций, не смешанный с возможностями открытого драйвера.
- [Охват](coverage.json), [снимок таблицы OpenRGB](source-data/openrgb-pipeline.csv),
  [атрибуция и лицензии](NOTICE.md).

После сборки: **1 526 записей источников**: OpenRGB — 1 024, OpenRazer — 274,
ckb-next — 89, libratbag — 124, документация производителей — 15.
Это **не количество уникальных моделей**: один товар может присутствовать в
нескольких источниках и иметь разные USB/радио-варианты. Записи намеренно не
склеены по похожему торговому названию. Полного перечня всех когда-либо выпущенных
моделей и их ревизий здесь пока нет.

Охвачены Logitech, Razer, Corsair, SteelSeries, ASUS, HyperX, ROCCAT/Turtle Beach,
MSI, Cooler Master, Glorious, Keychron, Wooting, A4Tech/Bloody и пример Gembird.
Для неизвестной клавиатуры пользователя Gembird точное соответствие не установлено.
Каталог содержит также других производителей и записи с общим OEM VID.

## Как устроен доступ к устройству

Обычный ввод, расширенная настройка и обработка звука — разные пути.

1. **Обычный ввод.** Мышь/клавиатура передаёт стандартные HID input reports ОС.
   В Windows основа — `hidclass.sys` и транспортный стек; приложение производителя
   может работать с отдельной коллекцией HID или через собственный компонент.
   Поэтому «есть фирменная программа» не означает «нужен свой kernel-драйвер».
   Стандартные коллекции мыши/клавиатуры Windows открывает для ввода эксклюзивно;
   для настройки нужно находить соответствующий интерфейс устройства.
   [Microsoft: архитектура](https://learn.microsoft.com/en-us/windows-hardware/drivers/hid/hid-architecture),
   [варианты клиента](https://learn.microsoft.com/en-us/windows-hardware/drivers/hid/hid-client-drivers).
2. **Настройка периферии.** DPI, назначения, RGB, питание и профили часто идут по
   vendor-defined HID либо другим нестандартным сообщениям. Дескриптор описывает
   формат отчётов, но не раскрывает смысл произвольной команды производителя.
   Linux `hidraw` передаёт сырые отчёты; знание протокола всё равно необходимо.
   [USB-IF](https://www.usb.org/hid),
   [Linux hidraw](https://www.kernel.org/doc/html/latest/hid/hidraw.html).
3. **Программный SDK.** Приложение отправляет команды сервису/ПО производителя,
   который общается с оборудованием. Это полезный дополнительный путь, но требует
   установленного ПО и не даёт всех функций его интерфейса автоматически.
4. **Аудио.** Гарнитура может объединять USB Audio, HID-настройку и обработку на
   компьютере. EQ/пространственный звук могут работать в программном обработчике,
   а не в памяти наушников. Нельзя переносить такой эффект как «аппаратный профиль»
   без проверки. Windows имеет USB Audio 2.0 class driver; APO — отдельный механизм
   обработки звука. [USB Audio](https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/usb-2-0-audio-drivers),
   [APO](https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/audio-processing-object-architecture).

USB, 2.4 ГГц через приёмник и Bluetooth одной модели считаются отдельными
маршрутами. VID/PID приёмника не обязательно идентифицирует подключённую мышь;
часть идентификаторов в libratbag относится к сопоставлению протокола. Название,
VID/PID и совпадение длины отчёта сами по себе не разрешают запись.

## Производители: ПО, параметры и известные пути

В таблице — семейства функций. **Набор конкретной модели смотри в каталоге**;
общая строка производителя не назначает все эти возможности каждому товару.

| Производитель | ПО и функции по публичным материалам | Открытый путь и границы |
|---|---|---|
| Logitech | G HUB: DPI, назначения, макросы, LIGHTSYNC, память на поддержанных устройствах | libratbag реализует HID++ 1.0/2.0. HID++2 обнаруживает feature index во время работы. LED SDK требует G HUB и не является универсальным API DPI. [G HUB](https://www.logitechg.com/en-us/software/ghub), [SDK](https://www.logitechg.com/en-us/programs/partner-developer-lab) |
| Razer | Synapse: DPI, polling, назначения/Hypershift, Chroma; калибровка сенсора и аналоговые функции зависят от модели | OpenRazer: Linux kernel-модули + daemon + клиентский интерфейс. По классам извлечены конкретные методы и пределы. Наличие Synapse-функции не гарантирует её наличие в OpenRazer. [Synapse](https://www.razer.com/synapse-4), [проект](https://github.com/openrazer/openrazer) |
| Corsair | iCUE: настройки периферии, эффекты, назначения, DPI; доступность зависит от поколения | iCUE SDK управляет светом через запущенный iCUE. ckb-next содержит собственные реализации для моделей/поколений Corsair; OpenRGB — отдельные RGB-контроллеры. Команды ckb-next нельзя автоматически считать временными и разрешёнными UniDevice. [SDK](https://corsairofficial.github.io/cue-sdk/), [ckb-next](https://github.com/ckb-next/ckb-next/wiki) |
| SteelSeries | GG Engine: CPI/DPI, кнопки, подсветка; на Apex Pro — actuation/Rapid Trigger; на гарнитурах — аудиопараметры | GameSense предоставляет события для света, OLED и haptics, не общий редактор CPI и actuation. Для ряда мышей есть libratbag. [GameSense](https://steelseries.com/developer), [исходники SDK](https://github.com/SteelSeries/gamesense-sdk) |
| ASUS ROG/TUF | Armoury Crate / Gear: производительность мыши, назначения, калибровка, свет, питание; Aura Sync/Creator для подсветки | libratbag содержит модельные описания ASUS: DPI, число ступеней, кнопок, профилей и quirks. RGB имеет отдельные реализации OpenRGB. Список совместимости Armoury Crate не раскрывает USB-протокол. [Функции](https://rog.asus.com/content/armoury-crate/), [список](https://rog.asus.com/support/faq/1042459/) |
| HyperX | NGENUITY и NGENUITY Legacy имеют разные списки устройств; настройка мышей/клавиатур/аудио | OpenRGB даёт сведения о подсветке части моделей, но не доказывает управление DPI/EQ. Важны SKU и версия NGENUITY. [Официальные списки](https://hyperx.com/pages/ngenuity/text-link) |
| ROCCAT / Turtle Beach | Swarm/Swarm II: назначения, подсветка, функции радиосвязи, макросы, EQ по устройству | Есть описания ряда ROCCAT в libratbag и RGB в OpenRGB. Бренд-преемник и похожий корпус не доказывают совместимость протоколов. [Swarm II](https://support.turtlebeach.com/s/article/what-is-swarm-ii?language=en_US) |
| MSI | MSI Center / Gaming Gear для DPI, Mystic Light для света | Настраиваемый DPI подтверждён официальной FAQ конкретных GM51. RGB-контроллеры OpenRGB исследуются отдельно от мышиной производительности. [DPI](https://us.msi.com/faq/nb-8764), [свет](https://us.msi.com/support/technical_details/DT_SW_Mystic_Light) |
| Cooler Master | MasterPlus+: настройки поддержанных устройств; для MM712 показаны конкретные DPI/LOD и различия транспорта | Есть RGB-реализации OpenRGB; широкого публичного API всех функций в исследованных материалах не установлено. [MM712 Sakura](https://www.coolermaster.com/nl-nl/products/mm712-sakura-limited-edition.html) |
| Glorious | CORE и отдельное ПО старых моделей; кнопки, RGB, debounce, sensor options | libratbag содержит Sinowealth-профиль первого Model O/O− с проверкой версии. Его нельзя переносить на Model O 2/PRO. [O 2 Wired](https://www.gloriousgaming.com/products/model-o-2-wired-mouse) |
| Keychron | Launcher; на совместимых HE-моделях actuation/Rapid Trigger, на поддержанных моделях keymap, macros, RGB | Механические QMK/VIA и HE не взаимозаменяемы. QMK Raw HID — транспорт, не универсальный набор команд. В HE guide описано автоматическое сохранение настроек. [Launcher](https://launcher.keychron.com/), [HE](https://www.keychron.com/pages/how-to-use-he-mode-on-keychron-launcher), [Raw HID](https://docs.qmk.fm/features/rawhid) |
| Wooting | Wootility: actuation, Rapid Trigger, DKS, layers, analog, RGB | Официальные RGB SDK и Analog SDK имеют разные задачи: управление светом и чтение аналогового ввода. Они не доказывают наличие setter API для всей Wootility. [RGB](https://github.com/WootingKb/wooting-rgb-sdk), [Analog](https://github.com/WootingKb/wooting-analog-sdk) |
| A4Tech/Bloody | В выбранной базе есть запись B820R для RGB | Это одна запись OpenRGB, не исследование всего Bloody ПО и всех фирменных функций. В этой части охват пока ограничен. |
| Gembird | У Firebolt описаны аппаратная кнопка DPI и выключатель RGB; у моделей клавиатур параметры индивидуальны | Публичный host API по рассмотренному товару не установлен. Это не доказательство отсутствия такого API у всех Gembird. Неизвестную клавиатуру пока нельзя классифицировать. [Пример Firebolt](https://www.gembird.com/item.aspx?id=12089&lang=es) |

## Примеры различий между моделями

| Модель | Подтверждённые сведения | Чего из них не следует |
|---|---|---|
| PRO X SUPERLIGHT 1 | HERO, 100–25 600 DPI, максимум 1 000 Гц | RGB и характеристики HERO 2 не добавляются по аналогии. [Logitech](https://www.logitechg.com/en-us/products/gaming-mice/pro-x-superlight-wireless-mouse.html) |
| G502 HERO | До 25 600 DPI, до пяти ступеней, DPI Shift, 11 программируемых кнопок, LIGHTSYNC | Внутри страницы противоречие минимального DPI: 200 в обзорном тексте и 100 в sensor details. Нижнюю границу следует запросить у устройства. [Logitech](https://www.logitechg.com/en-us/shop/p/g502-hero-gaming-mouse.910-005469) |
| DeathAdder V2 | До 20 000 DPI, polling, восемь назначаемых кнопок, Chroma, lift-off/landing settings | OpenRazer не экспортирует всю функциональность Synapse для этой модели; оба слоя сохранены отдельно. [Razer](https://mysupport.razer.com/app/answers/detail/a_id/3717/~/razer-deathadder-v2-%7C-rz01-03210-support) |
| Harpoon RGB PRO | По прежнему тесту проекта: firmware 3.13, временные DPI и цвет активной ступени | Сведения ckb-next/OpenRGB о других версиях не расширяют действующий allowlist проекта. Текущего физического теста нет. [Отчёт проекта](../reports/studio-v03-hardware.json) |
| GM51 LIGHTWEIGHT / WIRELESS | Проводная: 8 000 Гц; беспроводная: 1 000 Гц; обе до 26 000 DPI | Одинаковое семейство и сенсор не означают одинаковую частоту. [Wired](https://www.msi.com/Gaming-Gear/CLUTCH-GM51-LIGHTWEIGHT/Specification), [Wireless](https://us.msi.com/Gaming-Gear/CLUTCH-GM51-LIGHTWEIGHT-WIRELESS/Specification) |
| Apex Pro Gen 3 | RGB/keybindings доступны по всей клавиатуре; actuation/dual actuation — на OmniPoint | Нельзя показывать actuation для обычных Red-switch клавиш. [SteelSeries](https://support.steelseries.com/hc/en-us/articles/33719007261709-What-type-of-switches-does-Apex-Pro-Gen-3-have) |
| Q1 HE 8K | Проводная HE: 8 000 Гц, actuation 0,1–3,35 мм, Rapid Trigger, remap/macros/RGB | Не та же модель, что Q1 HE wireless или механическая Q1. [Keychron](https://www.keychron.com/collections/usb-c-keyboards/products/keychron-q1-he-8k-magnetic-switch-keyboard) |
| Arctis Nova Pro Wireless | EQ 10 bands, gain, sidetone, mic volume; surround требует ПО на PC | Наличие настройки в меню базы не раскрывает host-команду; программный surround не является общим аппаратным параметром. [Руководство](https://downloads.steelseriescdn.com/guides/Arctis_Nova_Pro_WL_PC_PIG_RC-2.pdf) |

## Как читать данные

`manufacturer_documented_not_protocol_verified` — функция или характеристика
заявлена в документации производителя. Поле `specifications_not_all_adjustable`
содержит также фиксированные характеристики; максимум частоты не доказывает
наличие переключателя или конкретного списка значений.

`upstream_source_declared` — сведения открытого проекта. У OpenRazer сохранены
наследуемые методы `get_*` / `set_*`, DPI_MAX, матрица и POLL_RATES. Если частота
взята из default daemon, это подписано. У libratbag сохранены только явно
объявленные модельные поля; пропуски остаются неизвестными. У ckb-next — список
зарегистрированных символов/ID и только явно перечисленные максимумы DPI.

У OpenRGB сохранены исходные `Direct` / `Effects` / `Save`, включая partial,
problematic и automatic. Они могут быть общими для контроллера. Категория также
может быть общей или ошибочной: например, запись мыши может иметь несколько
категорий контроллера. Ни категория, ни зелёный признак не заменяют модельный тест.
CSV Pipeline — отдельный плавающий источник; он **не приписан** commit SHA
зеркала OpenRGB. Его байты сохранены и проверяются SHA-256.

`unidevice_runtime_support=not_asserted` и `execution_permitted=false` стоят у
каждой записи. Ни одна запись каталога не установлена как исполняемый пакет
совместимости и не добавляет разрешённые операции в программу.

## Что это меняет в развитии UniDevice

Практическая основа автоадаптации — определить **семейство протокола и его версию**,
затем безопасно запросить функции. Для HID++2 это уже есть в ограниченном виде.
Список названий помогает распознаванию и объяснениям, но не расшифровывает
неизвестные команды. libratbag прямо разделяет добавление модели известного
протокола и reverse engineering нового протокола.
[Описание подхода libratbag](https://github.com/libratbag/libratbag#adding-devices-to-libratbag).

Предлагаемый порядок следующей реализации:

1. Нормализованный профиль возможностей: параметр, read/write, область действия,
   единицы, min/max/step/enum, транспорт, firmware, способ подтверждения, источник.
   Неизвестная граница остаётся неизвестной; UI показывает только доказанное.
2. Разделение backend: собственный проверенный протокол, необязательный внешний
   RGB SDK/сервис и чисто программный эффект. Пользователь видит зависимости.
3. Расширение HID++ runtime discovery с сохранением запрета onboard/firmware.
   Дополнительные интерфейсы приёмника требуют отдельной спецификации маршрута.
4. Razer как следующая крупная семья: начать с identity/read-only и моделирования
   транзакций по конкретным версиям; не переносить GPL-реализацию в MIT-код
   автоматически. Затем Corsair по поколениям и ASUS/SteelSeries по ограниченным
   модельным профилям. RGB всех брендов можно рассматривать через внешний backend,
   но это отдельный компромисс по зависимостям и проверяемости.
5. Без железа: сохранённые обезличенные дампы, fake transport, точные request/response
   fixtures, тесты отказов, несоответствия firmware, маршрута и размеров. Эмуляция
   проверяет реализацию, но не доказывает работоспособность физического устройства.

Для каждой новой операции остаётся обязательной классификация побочных эффектов:
временное изменение, постоянная память, неопределённое сохранение, reset/DFU.
Неясная операция остаётся заблокирована. Автоматическое сохранение настроек,
документированное у части клавиатур, особенно важно для политики ресурса памяти.
Функция сохранения сама по себе не доказывает, что каждый цветовой кадр пишет flash.

## Пробелы и воспроизводимость

Не закрыты: все retail SKU/ревизии, пределы и шаги для каждой настройки каждой
модели, протоколы новейших закрытых устройств, точное расположение EQ/DSP каждого
аудиоустройства, вся линейка A4Tech/Gembird, закрытые службы фирменного ПО и
физическое подтверждение новых семейств. Для части устройств известна только RGB
часть. Поддержка firmware update в фирменной программе не входит в план UniDevice.

Для воспроизведения нужны Python 3.11+, Git и четыре checkout под
`local/research/{libratbag,openrazer,ckb-next,OpenRGB}`. Их URL и точные commit SHA
сохранены в `device-catalog.json -> sources`; перед повторной генерацией checkout
должны быть на этих commit. Скрипт не загружает и не запускает исходники драйверов:
OpenRazer читается через Python AST, C-таблицы — как текст.

```powershell
python scripts/Build-Research-Catalog.py
python scripts/Check-Research-Catalog.py
```

Проверка контролирует уникальность записей, наследование OpenRazer, сохранение
исходных RGB-флагов, отсутствие придуманных DPI-лимитов, различие GM51, SHA-256
таблицы, согласованность JSON/CSV/HTML и отсутствие разрешений на запись.
