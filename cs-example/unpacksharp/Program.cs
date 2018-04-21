using System;
using System.Runtime.InteropServices;
using System.Text;

namespace unpacksharp
{
	class MainClass
	{
		[DllImport("libv8unpack4rs.so", CharSet = CharSet.Ansi)]
		private static extern bool parse_cf(
			string fileName,
			string dirName);

		public static void Main(string[] args)
		{
			if (args.Length < 3)
			{
				Usage();
			}

			string command = args[0];

			if (command == "--parse")
			{
				string fileName = args[1];
				string dirName = args[2];
				
				var resultParse = parse_cf(fileName, dirName);
				Console.WriteLine("parse_cf: {0}", resultParse);
				
			}
			else
			{
				Usage();
			}
			
			//Console.ReadLine();
		}

		private static void Usage()
		{
			Console.WriteLine("unpacksharp --parse file_name dir_name");
			Environment.Exit(-1);
		}
	}
}
