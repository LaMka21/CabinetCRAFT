@Echo Off
Echo Vytvorenie PGMX suborov pre SCM
call "C:\Program Files (x86)\Scm Group\Maestro\XConverter.exe" -s -ow -m 0 -i "C:\Vyroba\CX100\SP1-BOTTOM.xcs" -t "C:\Program Files (x86)\Scm Group\Maestro\Tlgx\COLL.tlgx" -o "C:\Vyroba\CX100\SP1-BOTTOM.pgmx" -report
call "C:\Program Files (x86)\Scm Group\Maestro\XConverter.exe" -s -ow -m 0 -i "C:\Vyroba\CX100\SP1-STR-F.xcs" -t "C:\Program Files (x86)\Scm Group\Maestro\Tlgx\COLL.tlgx" -o "C:\Vyroba\CX100\SP1-STR-F.pgmx" -report
call "C:\Program Files (x86)\Scm Group\Maestro\XConverter.exe" -s -ow -m 0 -i "C:\Vyroba\CX100\SP1-STR-R.xcs" -t "C:\Program Files (x86)\Scm Group\Maestro\Tlgx\COLL.tlgx" -o "C:\Vyroba\CX100\SP1-STR-R.pgmx" -report
call "C:\Program Files (x86)\Scm Group\Maestro\XConverter.exe" -s -ow -m 0 -i "C:\Vyroba\CX100\HO1-SIDE-L.xcs" -t "C:\Program Files (x86)\Scm Group\Maestro\Tlgx\COLL.tlgx" -o "C:\Vyroba\CX100\HO1-SIDE-L.pgmx" -report
call "C:\Program Files (x86)\Scm Group\Maestro\XConverter.exe" -s -ow -m 0 -i "C:\Vyroba\CX100\HO1-SIDE-R.xcs" -t "C:\Program Files (x86)\Scm Group\Maestro\Tlgx\COLL.tlgx" -o "C:\Vyroba\CX100\HO1-SIDE-R.pgmx" -report
call "C:\Program Files (x86)\Scm Group\Maestro\XConverter.exe" -s -ow -m 0 -i "C:\Vyroba\CX100\HO1-BOTTOM.xcs" -t "C:\Program Files (x86)\Scm Group\Maestro\Tlgx\COLL.tlgx" -o "C:\Vyroba\CX100\HO1-BOTTOM.pgmx" -report
call "C:\Program Files (x86)\Scm Group\Maestro\XConverter.exe" -s -ow -m 0 -i "C:\Vyroba\CX100\HO1-TOP.xcs" -t "C:\Program Files (x86)\Scm Group\Maestro\Tlgx\COLL.tlgx" -o "C:\Vyroba\CX100\HO1-TOP.pgmx" -report
