#include <stdio.h>
#include "libqcs.h"

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
      const ExecutionData *ro = get_data(result->handle, "ro");
      for (int shot = 0; shot < ro->number_of_shots; shot++)
      {

         for (int qubit_index = 0; qubit_index < ro->shot_length; qubit_index++)
         {
            printf("%d", ro->data.byte[shot][qubit_index]);
         }
         printf(">\n");
      }
   }
}

bool get_readout_bit(ExecutionResult *result, int64_t shot_index, int64_t readout_index)
{
   const ExecutionData *ro = get_data(result->handle, "ro");

   if (ro == NULL) {
      printf("no data\n");
      exit(1);
   }

   if (ro->data.tag != DataType_Byte) {
      printf("data not of type byte");
      exit(1);
   }

   bool bit = (ro->data.byte[0][0]);
   return bit;
}