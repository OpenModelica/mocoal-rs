/*
This file is part of ModelicaComponentAutolayout.jl.
Copyright (c) 2026 Open Source Modelica Consortium (OSMC)

This program is provided under the terms of the OSMC Public License (OSMC-PL) version 1.8 or
the GNU AGPL version 3 license, according to the recipient's choice. See LICENSE.md.
*/

/* Test of the shared library libmocoal: test_lib MODEL.mo (see check.sh). */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

char *mocoal_layout(const char *text, const char *file_name, const char *options);
const char *mocoal_message(void);
const char *mocoal_version(void);
void mocoal_free(char *text);

static int count(const char *text, const char *word) {
  int n = 0;
  for (const char *p = strstr(text, word); p; p = strstr(p + 1, word)) n++;
  return n;
}

static char *readFile(const char *name) {
  FILE *f = fopen(name, "rb");
  if (!f) return NULL;
  static char buffer[1 << 16];
  size_t n = fread(buffer, 1, sizeof(buffer) - 1, f);
  fclose(f);
  buffer[n] = '\0';
  return buffer;
}

int main(int argc, char **argv) {
  if (argc != 2) {
    fprintf(stderr, "usage: test_lib MODEL.mo\n");
    return 2;
  }
  const char *model = readFile(argv[1]);
  if (!model) {
    fprintf(stderr, "cannot read %s\n", argv[1]);
    return 2;
  }
  int failures = 0;
  printf("version: %s\n", mocoal_version());
  failures += mocoal_version()[0] == '\0';

  /* The default options: 2 + 3 components placed, 2 connections routed */
  char *out = mocoal_layout(model, argv[1], NULL);
  int ok = out != NULL && count(out, "Placement(transformation(origin=") == 3 && count(out, "Line(points=") == 2 &&
           mocoal_message()[0] == '\0';
  printf("layout: %s (%s)\n", ok ? "ok" : "FAILED", mocoal_message());
  failures += !ok;

  /* A second layout of the result changes nothing */
  if (out != NULL) {
    char *again = mocoal_layout(out, "", "");
    ok = again != NULL && strcmp(out, again) == 0;
    printf("second layout: %s\n", ok ? "ok" : "FAILED");
    failures += !ok;
    mocoal_free(again);
  }
  mocoal_free(out);

  /* Options as on the command line */
  out = mocoal_layout(model, NULL, "--route none --class Chain");
  ok = out != NULL && count(out, "Placement(transformation(origin=") == 3 && count(out, "Line(") == 0;
  printf("options: %s (%s)\n", ok ? "ok" : "FAILED", mocoal_message());
  failures += !ok;
  mocoal_free(out);

  /* Errors: NULL and a message; the next layout clears the message */
  out = mocoal_layout("model A Real x end A;", "a.mo", NULL);
  ok = out == NULL && strstr(mocoal_message(), "a.mo:1:") != NULL;
  printf("parse error: %s (%s)\n", ok ? "ok" : "FAILED", mocoal_message());
  failures += !ok;
  out = mocoal_layout(model, NULL, "--layout circle");
  ok = out == NULL && strstr(mocoal_message(), "circle") != NULL;
  printf("unknown option value: %s (%s)\n", ok ? "ok" : "FAILED", mocoal_message());
  failures += !ok;
  out = mocoal_layout(model, NULL, NULL);
  ok = out != NULL && mocoal_message()[0] == '\0';
  failures += !ok;
  mocoal_free(out);

  /* What is not a layout option: NULL and a message */
  const char *refused[] = {"--frobnicate", "--route", "--keep=1", "--scale 0", "--dry-run", "--quiet", "--version",
                           "--svg d", "x.mo"};
  for (size_t i = 0; i < sizeof(refused) / sizeof(refused[0]); i++) {
    out = mocoal_layout(model, NULL, refused[i]);
    ok = out == NULL && mocoal_message()[0] != '\0';
    printf("refused %s: %s (%s)\n", refused[i], ok ? "ok" : "FAILED", mocoal_message());
    failures += !ok;
    mocoal_free(out);
  }

  /* A text without classes comes back as it is: the way to ask whether an option is known */
  out = mocoal_layout("", NULL, "--graphics move");
  ok = out != NULL && out[0] == '\0';
  printf("empty text: %s\n", ok ? "ok" : "FAILED");
  failures += !ok;
  mocoal_free(out);

  /* A word in double quotes; a class that is not there gives the text as it was and a warning */
  out = mocoal_layout(model, NULL, "--class \"Chain\" -L \"/no such dir\"");
  ok = out != NULL && count(out, "Line(points=") == 2;
  printf("quoted words: %s (%s)\n", ok ? "ok" : "FAILED", mocoal_message());
  failures += !ok;
  mocoal_free(out);
  out = mocoal_layout(model, NULL, "--class Nowhere");
  ok = out != NULL && strcmp(out, model) == 0 && strstr(mocoal_message(), "Nowhere") != NULL;
  printf("no such class: %s (%s)\n", ok ? "ok" : "FAILED", mocoal_message());
  failures += !ok;
  mocoal_free(out);

  printf("%s\n", failures ? "FAILED" : "all ok");
  return failures ? 1 : 0;
}
