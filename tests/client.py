# this needs "ibmtts_host32.dll" in the same folder.

import ctypes
import time


# --- ECI  protocol ---
class ECICallbackReturn:
    eciDataNotProcessed = 0
    eciDataProcessed = 1
    eciDataAbort = 2


class AudioFileWriter:
    def __init__(self, filename="output_audio.pcm"):
        self.filename = filename
        self.file = None
        self.client = None # Referencia a SharedAudioClient para obtener el puntero

    def open_file(self):
        self.file = open(self.filename, "wb")
        print(f"[*] audio file opened: {self.filename}")

    def callback(self, hEngine, msg, lparam, pData):
        """
        This is the callback that we will pass to the fake ECI DLL. It will be called by the host process when there is audio data to write or when the index is reached.
        """
        # MSG 0: Wave data (Audio)
        # simulaten the client is busy so we can check if the host can wait correctly.
        time.sleep(0.4)
        if msg == 0:
            samples_count = lparam
            if samples_count > 0 and self.file:
                ptr = self.client.get_audio_buffer_ptr()
                audio_bytes = ctypes.string_at(ptr, samples_count * 2)
                self.file.write(audio_bytes)
            return ECICallbackReturn.eciDataProcessed

        # MSG 2: index reached, in our test we insert index 100 after all the text, so we can use this to know when the synthesis is finished and close the file.
        elif msg == 2:
            if self.file:
                self.file.flush()
                self.file.close()
                self.file = None
                print("[*] Synthesis completed. File closed.")
            return ECICallbackReturn.eciDataProcessed
        return ECICallbackReturn.eciDataProcessed


test_string = """very large text to test shared audio buffer.
Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.
more text to ensure we exceed 1kbytes in the shared memory buffer. This should be enough to test the truncation logic on the client side when reading from shared memory after the event is signaled.
I'm looking forward to seeing this in action and verifying that the client correctly reads the shared memory content up to the first null byte, demonstrating that the communication between the host and client is working as intended.
I want to determine if the callback blocks other functions from being called or if they can run concurrently. This will help me understand the behavior of the host when handling multiple requests and callbacks, and whether the client can still interact with the host while waiting for a callback to complete.
to determine this, I need a string of at least 6kb to ensure that the synthesis process takes enough time to allow for testing concurrent interactions. By sending this large text to the host, I can observe if the client can still send other requests or if it gets blocked until the synthesis is complete.
This test will provide insights into the host's concurrency model and how it manages long-running operations like synthesis, as well as the responsiveness of the client during such operations.
This is a very large text string intended to test the shared memory buffer and see if it correctly truncates to the buffer size. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.
this is a story about a developer named lucy who is working on a text-to-speech project. Lucy has been tasked with creating a client application that communicates with a host process using named pipes and shared memory. The goal is to send text data to the host for synthesis and receive audio output in return. Lucy starts by implementing a simple client that can connect to the host and send messages using the defined protocol. As Lucy tests the client, she realizes that she needs to handle larger text inputs and ensure that the shared memory buffer is used correctly. She writes a test function that sends a large text string to the host and waits for the synthesis to complete. During this time, Lucy also checks if the client can still send other requests or if it gets blocked. Through this process, Lucy learns about the concurrency model of the host and how to manage long-running operations while keeping the client responsive. In the end, Lucy successfully implements the client application that can communicate with the host, send large text inputs, and receive audio output without blocking other interactions. This project not only enhances Lucy's understanding of inter-process communication.
This another story is about James Potter, Harry's father, who was a skilled wizard and a member of the Order of the Phoenix. James was known for his bravery and loyalty, often putting himself in danger to protect his friends and family. He attended Hogwarts School of Witchcraft and Wizardry, where he was sorted into Gryffindor House. During his time at Hogwarts, James was a talented Quidditch player and was also known for his mischievous nature. He had a close group of friends, including Sirius Black, Remus Lupin, and Peter Pettigrew. James eventually married Lily Evans, and they had a son named Harry. Tragically, James and Lily were killed by Lord Voldemort when Harry was just a baby, but their sacrifice protected Harry and allowed him to survive. James's legacy lives on through Harry, who inherits his father's courage and determination in the fight against evil.
Harry Potter and Ron Weasley were best friends who attended Hogwarts School of Witchcraft and Wizardry together. They were both sorted into Gryffindor House and shared many adventures throughout their time at Hogwarts. hermione Granger, known as The Harry's best friend, was a brilliant witch who excelled in her studies and was always eager to learn. Ron, on the other hand, was known for his loyalty and sense of humor, often providing comic relief during their adventures. The trio faced numerous challenges together, including battling dark wizards, solving mysteries, and participating in the Triwizard Tournament. Their friendship was tested at times, but they always stood by each other and supported one another through thick and thin. Harry, Hermione and Ron's bond was unbreakable, and they remained close friends even after their time at Hogwarts ended, continuing to fight against evil forces in the wizarding world.
Sirius Black was a complex character in the Harry Potter series, known for his loyalty and bravery. He was a member of the Order of the Phoenix and a close friend of James Potter. Sirius was wrongfully imprisoned in Azkaban for betraying the Potters to Lord Voldemort, but he later escaped and proved his innocence. Despite his troubled past, Sirius remained fiercely protective of Harry and played a crucial role in the fight against Voldemort. He was also known for his rebellious nature and love for adventure, often taking risks to help those he cared about. Sirius's tragic fate ultimately highlighted the themes of sacrifice and redemption in the series.
"""
import _fakeEci
def test_add_text():
    h = _fakeEci.EciDLL(r"C:\Users\legionD\AppData\Roaming\nvda\addons\IBMTTS\synthDrivers\ibmtts\ECI.DLL")
    # get an eci instance
    eci_hand = h.eciNew()
    print('ECI handle:', eci_hand)
    writer = AudioFileWriter(f"audio_{eci_hand:x}.pcm")
    h.eciRegisterCallback(eci_hand, writer.callback)
    # set the buffer hence the callback.
    resp = h.eciSetOutputBuffer(eci_hand, 3300)
    print("buffer is set", resp)
    writer.client = h
    writer.open_file()
    resp = h.eciAddText(eci_hand, "hello world"[:].encode('utf-8') + b'\x00')
    print('add_text resp:', resp)
    resp = h.eciInsertIndex(eci_hand, 100)
    print('insert_index resp:', resp)
    resp = h.eciSynthesize(eci_hand)
    print('synth resp:', resp)
    """start = time.perf_counter()
    resp = call_fn(h, ECI_SYNCHRONIZE, eci_hand)
    print('synch resp:', resp)
    resp = call_fn(h, ECI_ADD_TEXT, eci_hand, b"hola")
    end = time.perf_counter()
    print('time to add text:', end - start)
    print('add_text resp:', resp)"""

    return (h, eci_hand)

