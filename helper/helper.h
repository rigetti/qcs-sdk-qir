/**
 * Copyright 2022 Rigetti Computing
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * <http://www.apache.org/licenses/LICENSE-2.0>
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 **/
#include <stdio.h>
#include "libqcs.h"

typedef struct ExecutableCache
{
   int length;
   Executable **executables;
} ExecutableCache;

ExecutableCache *create_executable_cache(int length)
{
   Executable **executables = (Executable **)calloc(length, sizeof(struct Executable *));
   ExecutableCache *cache = (ExecutableCache *)malloc(sizeof(struct ExecutableCache *));
   cache->length = length;
   cache->executables = executables;
   return cache;
}

void add_executable_cache_item(ExecutableCache *cache, int index, char *program)
{
   cache->executables[index] = executable_from_quil(program);
}

Executable *read_from_executable_cache(ExecutableCache *cache, int index)
{
   if (index >= cache->length)
   {
      printf("cache only holds %d executables but tried to read from index %d", cache->length, index);
      exit(1);
   }

   Executable *executable = cache->executables[index];

   if (executable == NULL)
   {
      printf("read executable from cache at index %d but it was null", index);
      exit(1);
   }

   return executable;
}

void free_executable_cache(ExecutableCache *cache)
{
   for (int i = 0; i < cache->length; i++)
   {
      free_executable(cache->executables[i]);
   }

   free(cache);
}

// Given an execution result, test it for an error code. If present, print the error message and exit.
void panic_on_failure(ExecutionResult *result)
{
   if (result->tag == ExecutionResult_Error)
   {
      printf("error in execution: %s\n", result->error);
      exit(1);
   }
   else
   {
      printf("execution successful\n");
   }
}

// Get the bit present at `ro[readout_index]` recorded in shot index `shot_index - 1`.
// NOTE: shot_index is 1-indexed while the data is stored in a 0-indexed array. Thus, to read out from the first shot, pass `shot_index=1`.
bool get_readout_bit(ExecutionResult *result, int64_t shot_index, int64_t readout_index)
{
   const ExecutionData *ro = get_data(result->handle, "ro");

   if (ro == NULL)
   {
      printf("no data\n");
      exit(1);
   }

   if (ro->data.tag != DataType_Byte)
   {
      printf("data not of type byte");
      exit(1);
   }

   if (shot_index < 1)
   {
      printf("shot data array is indexed from 1; shot index must be >= 1; got %lld", shot_index);
      exit(1);
   }

   if (shot_index > ro->number_of_shots)
   {
      printf("requested shot index %lld; only %d shots taken", shot_index, ro->number_of_shots);
      exit(1);
   };

   if (readout_index < 0)
   {
      printf("readout data array is indexed from 0; shot index must be >= 0; got %lld", shot_index);
      exit(1);
   }

   if (readout_index >= ro->shot_length)
   {
      printf("requested readout index %lld; only %d elements in `ro` register", readout_index, ro->shot_length);
      exit(1);
   }

   bool bit = (ro->data.byte[shot_index - 1][readout_index]);
   return bit;
}