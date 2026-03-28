systeminfo > hw.txt
wmic path win32_VideoController get name, adapterram >> hw.txt
wmic cpu get name >> hw.txt
