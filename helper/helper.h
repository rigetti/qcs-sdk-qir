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
      // const ExecutionData *ro = get_data(result->handle, "ro");
      // for (int shot = 0; shot < ro->number_of_shots; shot++)
      // {

      //    for (int qubit_index = 0; qubit_index < ro->shot_length; qubit_index++)
      //    {
      //       printf("%d", ro->data.byte[shot][qubit_index]);
      //    }
      //    printf(">\n");
      // }
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