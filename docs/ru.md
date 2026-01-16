## Установка
> [!info]
> Debian/Ubuntu

1. Установить rust [rust](https://rust-lang.org/tools/install/):
```bash
curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh
```

2. Установить компилятор:
```bash
sudo apt install build-essential
```

3. Настройка системы:
   Есть два варианты:
- Использовать программу с sudo (тип сокета - raw). Никаких изменений не требуется.
- Использовать без привилегий (тип сокета - dgram)
  Проверка:
```bash
sysctl net.ipv4.ping_group_range
```
Если вывод `1   0` использовать:
```bash
sudo sysctl -w net.ipv4.ping_group_range="0 1000"
```

4. Клонировать репозиторий:
```bash
git clone https://github.com/notarius1/unshelve.git
cd unshelve
```

5. Компиляция:
```
cargo build --release
```

## Запуск
Чтобы не устанавливать как сервис, можно воспользоваться tmux `sudo apt install tmux`

### Флаги запуска
```
./unshelve [OPTIONS] <COMMAND>

Commands:
   server-list  Список всех облачных серверов
   server-info  Информация о конкретном облачном сервере <SERVER_NAME>
   unshelve     Ручная разморозка облачного сервера <SERVER_NAME>
   start        Запуск мониторинга сервера, авто разморозка, если нет пинга <SOCKET_TYPE>
   help         Вывод справки
   
Options:
   -c, --config <CONFIG>  Путь до конфига. По умолчанию .env файл
   -h, --help             Вывод справки
   -V, --version          Вывод версии
```

Конфигурацию можно сохранить в `.env` файл, тогда при запуске не нужно будет указывать конфигурационный файл, например:
```bash
./unshelve server-list
# или, с указанием конфига
./unshelve -c myconfig server-list
```

Команды `server-info` и `unshelve` требуют указания имени или UUID сервера в опциях или конфигурационном файле, в переменной `SERVER_NAME`
```bash
./unshelve server-info MyServer
# или, при наличии SERVER_NAME в конфиге
./unshelve server-info
```

Мониторинг (команда `start`) по умолчанию запускается с использованием dgram сокета. Можно переназначить, указав тип сокета явно:
```bash
./unshelve start # тип сокета - dgram
# или
./unshelve start dgram
# или
sudo ./unshelve start raw
```

### Пример конфига или .env файла
```bash
# OS_* - Переменные для OpenStack 
OS_AUTH_URL="https://cloud.api/identity/v3"  
OS_IDENTITY_API_VERSION="3"  
OS_VOLUME_API_VERSION="3"  
OS_PROJECT_DOMAIN_NAME='123456'  
OS_PROJECT_ID='0123456789abcdef'  
OS_TENANT_ID='0123456789abcdef'  
OS_REGION_NAME='arctic-1'  
OS_USER_DOMAIN_NAME='123456'  
OS_USERNAME='User'  
OS_PASSWORD='Str0ngPa$$word'  
  
# Имя или UUID облачного сервера
SERVER_NAME='Cloud01'  
# IP адрес облачного сервера
PING_IP='1.1.1.1'  
# Интервал между ICMP запросами (в минутах)  
PING_INTERVAL_MINUTES='5'  
# Таймаут для ICMP запроса (в секундах)
PING_TIMEOUT_SECONDS='1'
```