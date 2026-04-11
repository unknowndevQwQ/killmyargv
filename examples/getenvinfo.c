#include <errno.h>
#include <inttypes.h>
#include <pthread.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

// #define __getchar 1

// i hate illumos's vsnprintf, forever
#ifdef __illumos__
#define DXP "0x%p"
#else
#define DXP "%p"
#endif

extern char **environ;
int main(int argc, char *argv[], char *envp[])
{

    int t_argc = (int)*(argv - 1);
    printf("argc: %d/%d(" DXP "/" DXP "), argv: " DXP "\n", argc, t_argc, (void *)&argc, (void *)&t_argc, (void *)argv);
    for (int i = 0; i < argc; ++i)
    {
        printf("argv[%d] " DXP ": %s, len: %zu, endl: " DXP "\n",
               i, (void *)argv[i], argv[i], strlen(argv[i]), (void *)&argv[i][strlen(argv[i])]);
    }
    printf("environ: " DXP "\nenviron[0]: " DXP "\nenvp: " DXP "\nenvp[0]: " DXP "\n",
           (void *)environ, (void *)*environ, (void *)envp, (void *)*envp);
    // sleep(3);
    for (size_t i = 0; environ[i] != NULL; i++)
    {
        printf("environ[%zu] " DXP ": %s, len: %zu, endl: " DXP "\n",
               i, (void *)environ[i], environ[i], strlen(environ[i]), (void *)&environ[i][strlen(environ[i])]);
        printf("envp[%zu] " DXP ": %s, len: %zu, endl: " DXP "\n",
               i, (void *)envp[i], envp[i], strlen(envp[i]), (void *)&envp[i][strlen(envp[i])]);
    }

    char *heap_str = NULL;
    char stack_str[7] = "stack";
    char *rodata_str = "rodata";
    heap_str = (char *)malloc(sizeof(char) * 7);
    printf("random stack: " DXP ",random heap: " DXP ", random rodata: " DXP "\n",
           (void *)stack_str, (void *)heap_str, (void *)rodata_str);

    uintptr_t ptr_from_input = 0;
    intptr_t ptr_offset = 0;
    char input_char[4] = "";
    char cmd[99] = "";
#ifdef __illumos__
    // for solaris/illumos, maybe we don't have to use `/usr/ucb/ps -w`, try use `ps -F`
    char *ps = "ps -F -o user,pid,ppid,tty,s,time,comm,args -p";
#else
    /*
    假设只有 solaris/illumos 的 ps 如此难堪（指需要加 -p）
    当然，我不是说 coreutils 也是很好的。他们的 -p （ps awwxu -p $pid）不能按预期过滤/选择 pid 了，要使用 -q
    */
    char *ps = "ps -o user,pid,ppid,tty,stat,time,comm,args";
#endif
    system("type ps");

#ifndef not_default_change
    printf("ps output before all default change\n");
    sprintf(cmd, "%s %lld", ps, getpid());
    system(cmd);
    // pthread_setname_np();

    size_t argv0len = strlen(argv[0]);
    printf("argv[0] change pre: %s, len: %zu\n", argv[0], argv0len);
    memset(&argv[0][argv0len], '-', 1);
    printf("current argv[0]: %s\n", argv[0]);
    printf("ps output after kill endl null\n");
    system(cmd);

    char argv_0_1 = argv[0][1];
    memset(&argv[0][1], 0, 1);
    printf("current argv[0]: %s\n", argv[0]);
    printf("ps output after in argv 0 new null\n");
    system(cmd);

    void *environ_last_ptr = environ[0] - 1;
    size_t argv_last_len = strlen(argv[argc - 1]);
    printf("argv[argc - 1][len]: " DXP ", environ[0] - 1: " DXP "\n", (void *)&argv[argc - 1][argv_last_len], environ_last_ptr);
    memset(environ_last_ptr, '-', 1);
    printf("ps output after kill env before start null\n");
    system(cmd);
    sleep(3);

    printf("revert all change\n");

    memset(&argv[0][argv0len], 0, 1);
    memset(&argv[0][1], argv_0_1, 1);
    memset(environ_last_ptr, 0, 1);
#endif

    printf("ps output before input\n");
    system(cmd);

    while (true)
    {

        int ret = 0;
        uint64_t fail_count = 0;
        while ((ret = scanf("%lx %li %1s", &ptr_from_input, &ptr_offset, &input_char[0])) != EOF)
        {

            if (errno)
            {
                perror("scanf() err");
            }
            printf("scanf return: %d, errnum: %d, count: %ld\n", ret, errno, fail_count);
            if (ret > 0)
            {
                ret = 0;
                // Known issue: macos scanf() returns operation timeout error
#ifdef __getchar
                /*
                clean stdin buff
                form:
                http://hdongq.blog.163.com/blog/static/169402621201021011158231/ or
                https://juejin.cn/post/7130420046597193759
                */
                while ((input_char[3] = getchar()) != '\n' && input_char[3] != EOF)
                {
                    printf("0x%x\n", input_char[3]);
                };
#else
                // with scanf() from https://www.cnblogs.com/zjuhaohaoxuexi/p/16260800.html
                scanf("%*[^\n]");
                scanf("%*c");
#endif
                break;
            }
            else
            {
                sleep(1);
                fail_count += 1;
            }
        }

        printf("input addr: " DXP ", offset: %li, char: %c\n",
               (void *)ptr_from_input, ptr_offset, input_char[0]);

        void *try = (void *)(ptr_from_input + ptr_offset);
        printf("try: " DXP "\n", try);
        if (input_char[0] == '~')
        {
            input_char[0] = '\0';
        }
        memset(try, (int)input_char[0], 1);

        printf("ps output after input\n");
        // sprintf(cmd, "%s %lld", ps, getpid());
        system(cmd);
        sleep(3);
    }

    // printf("comm: %s\n", getprogname());
    // sleep(1);
    // setprogname("whoami");
    // printf("comm: %s\n", getprogname());
    // sleep(3);
}
