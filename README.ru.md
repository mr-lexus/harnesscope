# Harnesscope

[English version](README.md)

> **Локальная ретроспектива работы AI-агентов: наблюдение, оценка результата, улучшение workflow.**

**Данные для ретроспективы:** вкладка Evidence показывает очищенные наблюдения,
хронологию задач, оценки результата и версии workflow с различиями. Добавлены
переносимый экспорт, read-only MCP, hooks Codex и локальный OTLP/HTTP JSON.
См. [подключение и API](docs/EVIDENCE.ru.md) и [матрицу возможностей](docs/CAPABILITIES.ru.md).
Основная цель — Codex GUI на macOS; реальная приёмка на этой платформе ещё впереди.

Harnesscope записывает наблюдения о запусках агентов в локальную SQLite-базу и помогает оценивать результаты и сравнивать эксперименты над workflow. Есть обёртки Codex/Copilot/OpenCode, универсальный `run` для других команд и импорт событий JSON/JSONL. Сам Harnesscope не вызывает LLM.

Обёртка наблюдает **весь процесс (`PROCESS`)**, а не отдельные запросы внутри него. Для `TURN` нужен адаптер с подтверждёнными границами запроса. Код выхода 0 не означает, что результат принят человеком. GUI-launcher может завершиться раньше самого редактора.

---

## Установка

### Homebrew (macOS и Linux)
```bash
# macOS: готовый бинарник, без обновления Xcode и локальной сборки
brew install --cask mr-lexus/tap/harnesscope
harnesscope server start

# Linux (или formula на macOS с совместимыми developer tools)
brew install --formula mr-lexus/tap/harnesscope
brew services start mr-lexus/tap/harnesscope
```

Выберите один способ. [Переход, обновления и версии выпусков](docs/HOMEBREW.ru.md).

### Скрипт прямой установки
```bash
# macOS и Linux:
curl -fsSL https://raw.githubusercontent.com/mr-lexus/harnesscope/main/install.sh | bash

# Windows (PowerShell):
irm https://raw.githubusercontent.com/mr-lexus/harnesscope/main/install.ps1 | iex
```

### Сборка из исходников
```bash
cd web && npm ci && npm run build && cd ..
cargo build --release
```

---

## Быстрый старт

### 1. Запуск рабочих сред агентов
Запускайте агенты через Harnesscope. Стандартные потоки ввода/вывода (stdin, stdout, stderr), интерактивный терминал TTY и коды завершения передаются прозрачно:

```bash
# Интерактивные CLI / TUI среды
harnesscope codex
harnesscope copilot
harnesscope opencode

# Любые редакторы, студии и CLI-агенты
harnesscope run cursor .
harnesscope run code .
harnesscope run --runner aider aider
```

*Примечание: При запуске любого агента Harnesscope автоматически поднимает локальный сервер телеметрии в фоне, если он ещё не был запущен.*

### 2. Просмотр телеметрии и Web UI
```bash
# Открыть Web UI в браузере по умолчанию (http://127.0.0.1:4242)
harnesscope ui

# Управление сервером вручную
harnesscope server start
harnesscope server status
harnesscope server stop

# Проверка окружения и путей к агентам
harnesscope doctor
```

### 3. Разбор результатов

На странице **Overview** выберите период, проект и агента. Откройте выполнение, отметьте результат, добавьте заметку и название эксперимента. Сравнивайте похожие задачи и экспортируйте выбранный отчёт в JSON. Демо-данные по умолчанию исключены из обзора.

```bash
harnesscope ingest --file events.jsonl
```

Импорт принимает наш общий формат событий, а не произвольные логи поставщика. См. [контракт интеграции](docs/EVENTS.ru.md) и [пример](examples/events.jsonl).

### 4. Загрузка демо-данных (опционально)
```bash
harnesscope demo seed
```

---

## Ключевые принципы

- **Никаких вызовов LLM**: Harnesscope никогда не запрашивает нейросеть для определения моделей, сессий или инструментов. Все факты извлекаются строго детерминированно: из аргументов процесса, системного окружения, Git, конфигураций и событий адаптеров. Если факт неизвестен — сохраняется `UNKNOWN`.
- **Корректная модель идентичности**: Чётко разделяет **Runtime** (процесс ОС), **Session** (логический диалог) и **Execution** (наблюдение процесса или подтверждённый turn). Если сессия возобновляется после перезапуска процесса (`process stopped != session stopped`), Harnesscope сохраняет непрерывность сессии.
- **Git-контекст и отслеживание конфликтов**: Фиксирует снимки состояния Git до и после выполнения (HEAD SHA, ветка, dirty-статус, изменённые файлы, diff stat). Если два агента одновременно работают в одном worktree, атрибуция изменений помечается как `AMBIGUOUS`.
- **Локальность и приватность**: Все данные хранятся локально в SQLite (режим WAL). Известные форматы секретов и секретные JSON-поля маскируются перед записью; это best effort, а не гарантия обнаружения любых секретов. Полные конфиги сохраняются только при `HARNESSCOPE_CAPTURE_CONFIG=1`. Сервер разрешает только loopback и same-origin доступ из браузера.

---

## MCP трекеров задач

MCP трекера подключается к вашему агенту. Harnesscope по локальной конфигурации
распознаёт чтение задач в наблюдаемых вызовах и сохраняет версии ответов.
В репозитории нет встроенного провайдера или его учётных данных. Возьмите за основу
`examples/task-adapter.json` и выполните
`harnesscope task-adapters add /путь/к/локальной-конфигурации.json`.

## Документация

- **Установка и запуск:** [Homebrew](docs/HOMEBREW.ru.md) · [Ежедневная работа](docs/RUNNING.ru.md) · [Платформы и GUI](docs/CROSS_PLATFORM.ru.md)
- **Сбор данных:** [Подключение, API и MCP](docs/EVIDENCE.ru.md) · [История Codex](docs/NATIVE_SOURCES.ru.md) · [Возможности и ограничения](docs/CAPABILITIES.ru.md)
- **Интеграции:** [События агентов](docs/EVENTS.ru.md) · [Адаптеры трекеров задач](docs/TASK_ADAPTERS.ru.md)
- **Надёжность:** [Доставка и мониторинг](docs/DELIVERY.ru.md) · [Резервные копии](docs/BACKUPS.ru.md) · [Производительность](docs/PERFORMANCE.ru.md) · [Приёмка](docs/ACCEPTANCE.ru.md)
- **Разработка:** [Архитектура](docs/ARCHITECTURE.ru.md) · [Руководство разработчика](docs/DEVELOPMENT.ru.md) · [Направление продукта](docs/PRODUCT.ru.md) · [История аудита](docs/AUDIT.ru.md)

## Нативный сбор Codex

На странице **Sources** можно подключить выбранный rollout-файл или каталог. Сборщик внутри `serve` импортирует реальные turns, tool calls и наблюдаемые токены, сохраняет позицию чтения и продолжает после перезапуска. По умолчанию — только метаданные. Доступны пауза, диагностика файлов и отключение источника без удаления истории.

```sh
harnesscope sources add-codex --path /absolute/path/to/codex/sessions
harnesscope serve
```

[Инструкция, жизненный цикл и границы поддержки](docs/NATIVE_SOURCES.ru.md). Для сравнения нативных turns выбирайте `TURN` в Overview.


### Доставка и мониторинг

Обёртки сохраняют отредактированные события на диск до отправки и наблюдают дочерний процесс каждые 15 секунд. Сервер восстанавливает доставку после перезапуска. В компактной адаптивной панели появились Monitor, фильтры в URL, мобильные карточки и выбор плотности. См. [семантику heartbeat, команды восстановления и ограничения](docs/DELIVERY.ru.md).


### Резервные копии и большие истории

Команды: `harnesscope backup create --output snapshot.db`, `backup verify --file snapshot.db`, `backup restore --file snapshot.db --output restored.db`. Снимок учитывает WAL; восстановление всегда создаёт новый файл и ставит нативные сборщики на паузу. Ожидающие отправки события outbox, исходные логи и конфигурацию сохраняют отдельно. [Инструкция](docs/BACKUPS.ru.md).

Журнал загружается только при открытии Events, постранично и с поиском по всей истории. [Замеры на 10/100 тысячах событий, проверки восстановления и границы масштабирования](docs/PERFORMANCE.ru.md).


Готовый локальный сборщик: запуск, пауза, автозапуск Windows и расположение данных — [ежедневное использование](docs/RUNNING.ru.md).
