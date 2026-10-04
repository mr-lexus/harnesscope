# Руководство по Homebrew для Harnesscope

[Документация](../README.ru.md#документация)

Harnesscope распространяется через официальный тап [`mr-lexus/homebrew-tap`](https://github.com/mr-lexus/homebrew-tap).

## macOS: установка с сохранением Xcode 26

```sh
brew update
brew install --cask mr-lexus/tap/harnesscope
harnesscope --version
harnesscope server start
harnesscope ui
```

Cask скачивает готовый выпуск для Apple Silicon или Intel, проверяет SHA-256 и
подключает бинарник. Локальная компиляция не нужна: установка не требует Rust,
Node, Python или обновления Xcode/Command Line Tools. Версия и выбранный путь
Xcode не меняются. Требования самого Homebrew и бинарника к macOS сохраняются.

Formula без Homebrew bottle может проходить проверки инструментов сборки, даже
если внутри только копирует бинарник. Явный `--cask` использует другой путь
установки; `--force` не заменяет его. Согласно [документации Homebrew](https://docs.brew.sh/Common-Issues#missing-command-line-tools),
casks и bottles устанавливаются без developer tools. Точный диагноз ошибки на
конкретном Mac требует её текста.

Обновление: `harnesscope server stop`, затем
`brew upgrade --cask mr-lexus/tap/harnesscope`, затем `harnesscope server start`.
Сохраняйте настройки нестандартного пути базы при перезапуске.
Cask не настраивает автозапуск при входе. **`brew services` управляет formula,
а не этим cask.** Инструкции для formula ниже относятся к другому способу установки.

### Переход с установленной formula

Сначала сохраните резервную копию телеметрии. Не заменяйте новый сборщик из ветки
разработки более старым опубликованным бинарником. Остановите сервис и вручную
запущенный экземпляр, уберите ссылку на бинарник formula и установите cask:

```sh
brew services stop mr-lexus/tap/harnesscope
harnesscope server stop
brew unlink mr-lexus/tap/harnesscope
brew install --cask mr-lexus/tap/harnesscope
harnesscope server start
```

`unlink` сохраняет formula и пользовательские данные. Не используйте `--overwrite`
для обхода конфликтов. Для возврата остановите сборщик, удалите cask, выполните
`brew link mr-lexus/tap/harnesscope` и запустите сервис formula. Два сборщика не
должны использовать один порт/базу одновременно. Удаление cask не удаляет телеметрию.

### Без Homebrew

[Прямой установщик](../install.sh) также скачивает выпуск с проверкой контрольной
суммы в `~/.local/bin` без локальной компиляции:

```sh
curl -fsSL https://raw.githubusercontent.com/mr-lexus/harnesscope/main/install.sh -o /tmp/harnesscope-install.sh
bash /tmp/harnesscope-install.sh
```

Выбирайте один способ установки, чтобы не путать бинарники в PATH и сервисы.
Штатные проверки запуска macOS сохраняются; Gatekeeper и настройки Xcode не меняются.

### Какая версия устанавливается

Текущие cask/formula указывают на опубликованную **0.2.3**. Новый сборщик для
ретроспектив **0.3.0-rc.2** пока в ветке разработки и ещё не опубликован как
скачиваемый выпуск. Cask не устанавливает неопубликованные возможности.
Не заменяйте сборщик с новой базой более старой версией. См. [матрицу поддержки](CAPABILITIES.ru.md).
Релизный CI генерирует formula и cask из одного `checksums.txt` и обновляет tap
при настроенном токене. Предварительные выпуски не заменяют стабильный tap.

---

## 1. Установка formula (Linux или macOS с совместимыми developer tools)

```bash
# Подключение тапа и установка harnesscope
brew install --formula mr-lexus/tap/harnesscope
```

Проверка установки:
```bash
harnesscope --version
harnesscope doctor
```

---

## 2. Управление фоновым сервисом телеметрии

Homebrew позволяет запускать Harnesscope как фоновый системный демон (`launchd` на macOS, `systemd` на Linux):

```bash
# Автозапуск фонового сервера при входе в систему
brew services start mr-lexus/tap/harnesscope

# Проверка статуса сервиса
brew services list

# Остановка фонового сервера
brew services stop mr-lexus/tap/harnesscope

# Перезапуск сервиса
brew services restart mr-lexus/tap/harnesscope
```

При работе сервиса Homebrew:
- **Адрес Web UI и API**: `http://127.0.0.1:4242`
- **Логи**: `$(brew --prefix)/var/log/harnesscope.log`
- **Ошибки**: `$(brew --prefix)/var/log/harnesscope.error.log`

---

## 3. Обновление через Homebrew

```bash
brew update
brew upgrade mr-lexus/tap/harnesscope
brew services restart mr-lexus/tap/harnesscope
```

---

## 4. Запуск без системного демона

Если вы не хотите использовать `brew services`, Harnesscope имеет собственные кроссплатформенные команды:

```bash
# Открыть Web UI (при необходимости автоматически поднимает фоновый процесс)
harnesscope ui

# Управление демоном через CLI
harnesscope server start
harnesscope server status
harnesscope server stop
```
