# GLB Studio

Нативный desktop-просмотрщик GLB/glTF на **Rust + miniquad + macroquad**.
Windows, Linux и macOS используют OpenGL. На macOS используется профиль OpenGL 3.2
с GLSL 1.50. Metal-шейдеры сохранены, но Metal-backend miniquad 0.4.11 пока не
выбран: в нём не реализованы необходимые viewport/линии и MSAA resolve.
Модели обрабатываются локально. Сервер, браузер и подключение к интернету для работы не нужны.

## Запуск

Готовая Windows-сборка: `dist/glb-studio-windows-x64/GLB Studio.exe`.
Можно открыть приложение без аргументов, выбрать **Open GLB**, перетащить файл в окно
или передать путь через командную строку. При старте без файла показывается встроенный
текстурированный робот с 11 суставами и двумя анимациями.

```sh
cargo run --release
cargo run --release -- "path/to/model.glb"
```

Открываются `.glb` и `.gltf` с локальными ресурсами. Диалог выбора файла — системный;
разбор модели и декодирование текстур выполняются в рабочем потоке. Ошибка импорта
показывается в окне; предыдущая модель остаётся доступна.

## Логи и диагностика

Лог создаётся автоматически **до создания окна**, в том числе при запуске из Finder:

- **macOS:** `~/Library/Logs/GLB Studio/glb-studio.log`.
- **Windows:** `%LOCALAPPDATA%\GLB Studio\Logs\glb-studio.log`.
- **Linux:** `${XDG_STATE_HOME:-~/.local/state}/glb-studio/glb-studio.log`.

На Mac нажмите в Finder `Cmd+Shift+G` и вставьте `~/Library/Logs/GLB Studio/`.
При пустом окне или сбое пришлите файл `glb-studio.log` после воспроизведения ошибки.
Он содержит версию/commit сборки, ОС/архитектуру, графический backend, этапы
инициализации, импорт модели, первые кадры, сообщения об ошибках и panic backtrace.
На macOS/Linux в тот же файл перенаправляются stdout/stderr, включая сообщения
нативных графических библиотек. Содержимое моделей в лог не записывается.

Запуски дописываются в один файл. Если при следующем запуске он больше 2 MiB,
предыдущий лог сохраняется как `glb-studio.previous.log`.
Каталог можно задать через `GLB_STUDIO_LOG_DIR`; если основной каталог недоступен,
используется `GLB Studio` в системной временной папке.

## Управление

| Действие | Управление |
| --- | --- |
| Вращение вокруг точки фокуса | ЛКМ + движение мыши |
| Перемещение точки фокуса в плоскости экрана | ПКМ + движение мыши |
| Приближение / отдаление | Колесо |
| Вписать текущую позу в окно | `F` / Fit |
| Сбросить ракурс и вписать модель | `Home` |
| Открыть модель | `O`, `Ctrl+O` / Open GLB |
| Модель с материалом и освещением | `1` / Shaded |
| Только сетка, включая обратную сторону | `2` / Wire |
| Сетка поверх материала | `3` / Overlay |
| Скелет поверх модели | `S` / Skeleton x-ray |
| Сетка пола | `G` / Ground grid |
| Воспроизведение / пауза | `Space` / Play / Pause |
| Предыдущая / следующая анимация | Стрелки влево / вправо или кнопки `<` / `>` |
| Перемотка | Нажатие / перетаскивание ползунка таймлайна |

В списке анимаций также есть **Rest pose**. Поддержаны скорости 0.25–2x, Loop
и возврат к началу клипа. Перемотка автоматически ставит воспроизведение на паузу.
При ширине окна меньше 860 логических пикселей боковая панель скрывается;
горячие клавиши и таймлайн остаются доступны.

Колесо меняет дистанцию примерно на 4% за щелчок, с плавным переходом
около 0.25 секунды. Единицы прокрутки нормализуются для Windows/macOS/X11;
дробные движения тачпада сохраняются. `F` и `Home` сбрасывают накопленный зум.

## Отображение и формат

- Base color, vertex colors, metallic/roughness, normal, occlusion, emissive; PNG/JPEG.
- Три студийных источника света, полусферический заполняющий свет, регулировка
  экспозиции, преобразование linear/sRGB и тональная компрессия ACES approximation.
- Alpha MASK/BLEND, двусторонние материалы и `KHR_materials_unlit`.
- Иерархия узлов, матричные преобразования, индексированные и неиндексированные
  треугольники, triangle strip/fan; генерация нормалей при их отсутствии.
- Скелетная деформация до 8 влияний на вершину (`JOINTS_0/1`, `WEIGHTS_0/1`),
  inverse bind matrices, положение костей в текущей анимационной позе.
- Анимация translation/rotation/scale и morph weights; LINEAR, STEP, CUBICSPLINE;
  shortest-path slerp для линейной quaternion-анимации.
- Постоянные GPU-буферы через miniquad. Большие меши делятся на порции с локальными
  16-битными индексами. Статические модели не загружаются на GPU каждый кадр.

Первая версия использует CPU skinning: очень плотные анимированные модели
могут работать медленнее статических. Нормальные карты восстанавливают tangent basis
по производным. Реальных теней и HDR environment maps пока нет. Прозрачность
сортируется по центрам частей меша, поэтому пересекающиеся прозрачные поверхности
могут отображаться неточно.

Загружается default scene (либо первая сцена). Point/line primitives пропускаются.
У одного материала используется один набор UV: дополнительные карты с другим UV
отключаются с примечанием. Draco, meshopt, KTX2/Basis, texture transform и расширенные
материалы transmission/volume/clearcoat не реализованы. Неподдерживаемые **required**
extensions дают ошибку; optional extensions игнорируются с примечанием.
Максимальный размер изображения ограничен 16384 px; фактический GPU должен его поддерживать.

## Сборка на платформах

Установите актуальный stable Rust через [rustup](https://rustup.rs/).

- **Windows:** toolchain `*-pc-windows-msvc`, Visual Studio Build Tools с C++ workload
  и Windows SDK. `cargo build --release --locked`.
- **macOS:** Xcode Command Line Tools (`xcode-select --install`), затем та же команда.
  Используется встроенный OpenGL framework; отдельная установка драйвера не нужна.
- **Linux:** C toolchain, X11 и OpenGL/Mesa. В Debian/Ubuntu:
  `sudo apt install build-essential pkg-config libx11-dev libxi-dev libgl1-mesa-dev`.
  Для системного диалога нужны `xdg-desktop-portal` и backend своего окружения
  (например `xdg-desktop-portal-gtk` или KDE). Используется portal backend rfd,
  GTK development packages для сборки самого диалога не нужны.

Сборка и проверка на Windows выполнены локально.

Для Universal macOS-билда (Apple Silicon + Intel, macOS 11+) выполните на Mac:

```sh
brew install glslang
python3 tools/build_macos.py
```

Скрипт проверяет GLSL 1.50 и Metal-шейдеры без графического окна, собирает две архитектуры, объединяет их через
`lipo` и создаёт `dist/glb-studio-macos-universal.zip` с `GLB Studio.app`.
Приложение не запускается. Для локальной подписи используется ad-hoc codesign;
Developer ID и notarization в эту сборку не входят.

В GitHub Actions workflow **Build macOS app** выполняет тот же процесс при push
или ручном запуске. Готовый архив находится в артефакте **GLB-Studio-macOS-Universal**.

## Проверки и упаковка

```sh
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo run --release --bin glb-inspect -- "path/to/model.glb"
python tools/package.py
```

`glb-inspect` принимает несколько путей, проверяет импорт и рассчитывает каждый клип
в 0%, 25%, 50%, 75% и 100% длительности без создания окна. Код выхода ненулевой при ошибках.
Для автоматической проверки графики переменная окружения `GLB_VIEWER_SMOKE_FRAMES=90`
запускает ограниченный цикл с Shaded, Wire и Overlay + skeleton, после чего закрывает окно.

`python tools/make_demo.py` воспроизводит оригинальную демонстрационную GLB-модель.
Пользовательские тестовые модели не включены в репозиторий и дистрибутив.

## Структура

- `src/scene.rs`, `src/animation.rs`: CPU-импорт, позы, skinning, morphs.
- `src/render.rs`, `src/shaders/`: miniquad GPU-буферы и GLSL/MSL-шейдеры.
- `src/camera.rs`: orbit/pan/zoom и проекция скелета.
- `src/ui.rs`, `src/player.rs`: macroquad UI и управление воспроизведением.
- `src/main.rs`: окно, основной цикл, фоновый загрузчик.
- `src/diagnostics.rs`: файловый лог, ранняя диагностика и panic hook.
- `src/bin/glb-inspect.rs`: проверка моделей без окна.

Документация библиотек: [macroquad](https://docs.rs/macroquad/0.4.16/macroquad/),
[miniquad](https://docs.rs/miniquad/0.4.11/miniquad/),
[gltf](https://docs.rs/gltf/1.4.1/gltf/), [rfd](https://docs.rs/rfd/0.15.4/rfd/).
Noto Sans распространяется по SIL OFL; лицензия включена в `assets/OFL-NotoSans.txt`.
