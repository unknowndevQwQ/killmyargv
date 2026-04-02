# killmyargv

灵的来了嗷

一个尝试实现类似 libbsd/libnostd 库提供的 `setproctitle()` 函数的基础实现。但就像第一句所暗示的那样，还没有被严格测试

## 警告

本项目的行为依赖于主流 posix-like 系统~~自古以~~数十年来未更改且行为各异的 Undocumented 实现。

不喜欢这种实现方式的伙计们应该向正在使用的 OS 开发商发邮件要一个行为类似的 syscall，或学习[这位][freebsd_setcmdline]为自己在用的系统做一点小小的贡献。  
如果是有 syscall 而且不喜欢每个系统有各自的 syscall 命名和/或使用方式的伙计们应该穿越到 sendmail 刚发明创造自用的 `setproctitle()` 的那一天，或者穿越到四人帮在写 unix 的时候，至于什么目的、做什么大概没人管。~~如果有某位幸运伙计因回到过去成功地把 lisp 机器推广成为长到不知道多少年的全球第一大机或者使 lisp 机器自古以来的机器，也不知道得收获多少百分比的全人类感谢你~~



[freebsd_setcmdline]: https://man.freebsd.org/cgi/man.cgi?query=setproctitle&manpath=FreeBSD+15.0-RELEASE#AUTHORS "Peter Wemm <peter@FreeBSD.org>"