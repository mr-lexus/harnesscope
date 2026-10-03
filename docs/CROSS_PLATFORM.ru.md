> Current observation boundaries: wrappers track the launched process, not every internal turn or detached GUI session. Telemetry delivery has bounded waits, and failures do not stop the child. See [EVENTS.md](EVENTS.md) and [audit limitations](AUDIT.ru.md).

# Кроссплатформенная архитектура и запуск агентов

Harnesscope с самого начала спроектирован для нативной и легковесной работы на **macOS**, **Linux** и **Windows**.

---

## 1. Кроссплатформенная матрица

| Параметр | macOS (Intel / Apple Silicon) | Linux (x86_64 / arm64) | Windows (x64) |
|---|---|---|---|
| **Директория данных** | `~/Library/Application Support/com.harnesscope.harnesscope/` | `~/.local/share/harnesscope/` | `%LOCALAPPDATA%\harnesscope\harnesscope\data\` |
| **База данных** | SQLite WAL (встроенный C-движок, без внешних runtime-зависимостей) | То же | То же |
| **Нормализация путей** | Прямые слеши (`/`), поддержка Unicode | Прямые слеши (`/`), поддержка Unicode | Автоматическая конвертация `\` в `/` для Git-контекста |
| **Поиск программ** | PATH + `/opt/homebrew/bin` + `/Applications/*.app/Contents/MacOS/*` | PATH + `/usr/local/bin` + `~/.local/bin` + `/snap/bin` | PATH + `.exe`, `.cmd`, `.bat`, `.ps1` |
| **Запуск демона** | Отвязанный POSIX-процесс (`Stdio::null`) | Отвязанный POSIX-процесс (`Stdio::null`) | Флаги `DETACHED_PROCESS` + `CREATE_NO_WINDOW` |
| **Потоки ввода/вывода** | Наследуемый TTY (`Stdio::inherit`) | Наследуемый TTY (`Stdio::inherit`) | Консоль / Windows Terminal |

---

## 2. Запуск CLI и GUI сред

Вместо искусственных одноразовых запросов через командную строку, Harnesscope запускает **полноценные рабочие среды разработчика**:

### Прямые прозрачные обёртки
- `harnesscope codex [args...]`: Запуск интерактивного Codex CLI.
- `harnesscope copilot [args...]`: Запуск GitHub Copilot CLI.
- `harnesscope opencode [args...]`: Запуск OpenCode CLI/TUI.

### Универсальный запуск CLI и GUI (`harnesscope run`)
Для любых других инструментов, терминальных агентов или графических редакторов:

```bash
# Запуск Cursor Editor под наблюдением Harnesscope
harnesscope run cursor .

# Запуск VS Code с открытием текущего проекта
harnesscope run code .

# Запуск Aider или любого кастомного агента
harnesscope run --runner aider aider --model anthropic/claude-3-5-sonnet

# Явное указание GUI-режима
harnesscope run --gui my-agent-studio
```

### Автоматический запуск сервера
При вызове любой команды `harnesscope <runner>` или `harnesscope run ...`:
1. Утилита проверяет, работает ли локальный сервер на `127.0.0.1:4242`.
2. Если сервер не запущен, она **автоматически и незаметно поднимает фоновый процесс сервера**.
3. Фиксирует снимок Git и метаданные среды перед стартом.
4. Запускает целевую программу с сохранением терминального взаимодействия.
5. При завершении программы фиксирует итоговые изменения в Git и код завершения.
6. **Телеметрия никогда не ломает процесс агента**: если сервер по какой-либо причине недоступен, агент продолжает выполнение штатно.

---

## 3. Переменные окружения

- `HARNESSCOPE_DATA_DIR`: Путь к директории данных.
- `HARNESSCOPE_DB_PATH`: Путь к файлу базы данных SQLite.
- `HARNESSCOPE_SERVER_HOST`: Хост сервера (по умолчанию `127.0.0.1`).
- `HARNESSCOPE_SERVER_PORT`: Порт сервера (по умолчанию `4242`).
- `HARNESSCOPE_SERVER_URL`: URL сервера для отправки событий (по умолчанию `http://127.0.0.1:4242`).
- `HARNESSCOPE_CODEX_BIN`: Путь к исполняемому файлу Codex.
- `HARNESSCOPE_COPILOT_BIN`: Путь к исполняемому файлу Copilot.
- `HARNESSCOPE_OPENCODE_BIN`: Путь к исполняемому файлу OpenCode.
- `HARNESSCOPE_CURSOR_BIN`: Путь к исполняемому файлу Cursor.
- `HARNESSCOPE_CODE_BIN`: Путь к исполняемому файлу VS Code.
